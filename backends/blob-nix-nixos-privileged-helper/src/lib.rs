#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;

use blob_core::{
    NodeId, SystemAuthorityClass, SystemAuthorizationId, SystemCandidateAction, SystemEffectClass,
};
use blob_system_activation_gate::PreparedPrivilegedActivation;

pub const DEFAULT_EXECUTION_LEDGER_ROOT: &str = "/var/lib/theblob/privileged-executions";
pub const DEFAULT_MAX_PREPARED_AGE_MS: u64 = 30_000;
pub const DEFAULT_MAX_READINESS_AGE_MS: u64 = 120_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivilegedActivationRuntimePolicy {
    pub max_prepared_age_ms: u64,
    pub max_readiness_age_ms: u64,
}

impl Default for PrivilegedActivationRuntimePolicy {
    fn default() -> Self {
        Self {
            max_prepared_age_ms: DEFAULT_MAX_PREPARED_AGE_MS,
            max_readiness_age_ms: DEFAULT_MAX_READINESS_AGE_MS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivilegedActivationCommandStatus {
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivilegedCommandOutcome {
    pub status: PrivilegedActivationCommandStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_us: u64,
}

impl PrivilegedCommandOutcome {
    pub fn succeeded(&self) -> bool {
        self.status == PrivilegedActivationCommandStatus::Succeeded
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivilegedActivationExecution {
    pub node: NodeId,
    pub authorization: SystemAuthorizationId,
    pub action: SystemCandidateAction,
    pub before_system_closure: String,
    pub after_system_closure: String,
    pub command: PrivilegedCommandOutcome,
    pub rollback_attempted: bool,
    pub rollback_succeeded: bool,
}

impl PrivilegedActivationExecution {
    pub fn evidence_lines(&self) -> Vec<String> {
        vec![
            format!("node:{}", self.node),
            format!("authorization:{}", self.authorization),
            format!("action:{:?}", self.action),
            format!("before-system-closure:{}", self.before_system_closure),
            format!("after-system-closure:{}", self.after_system_closure),
            format!("command-status:{:?}", self.command.status),
            format!("command-duration-us:{}", self.command.duration_us),
            format!("rollback-attempted:{}", self.rollback_attempted),
            format!("rollback-succeeded:{}", self.rollback_succeeded),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrivilegedExecutionLedgerError {
    AlreadyConsumed(SystemAuthorizationId),
    Io(String),
    InvalidLedgerRoot,
}

pub trait PrivilegedExecutionLedger {
    fn consume_once(
        &self,
        authorization: &SystemAuthorizationId,
        prepared_at_unix_ms: u64,
    ) -> Result<(), PrivilegedExecutionLedgerError>;
}

#[derive(Default)]
pub struct InMemoryPrivilegedExecutionLedger {
    consumed: Mutex<BTreeSet<SystemAuthorizationId>>,
}

impl InMemoryPrivilegedExecutionLedger {
    pub fn was_consumed(&self, id: &SystemAuthorizationId) -> bool {
        self.consumed
            .lock()
            .map(|consumed| consumed.contains(id))
            .unwrap_or(false)
    }
}

impl PrivilegedExecutionLedger for InMemoryPrivilegedExecutionLedger {
    fn consume_once(
        &self,
        authorization: &SystemAuthorizationId,
        _prepared_at_unix_ms: u64,
    ) -> Result<(), PrivilegedExecutionLedgerError> {
        let mut consumed = self
            .consumed
            .lock()
            .map_err(|_| PrivilegedExecutionLedgerError::Io("execution ledger poisoned".into()))?;
        if !consumed.insert(authorization.clone()) {
            return Err(PrivilegedExecutionLedgerError::AlreadyConsumed(
                authorization.clone(),
            ));
        }
        Ok(())
    }
}

/// Durable single-use ledger for the privileged boundary.
///
/// The directory is deliberately *not* created here. Deployment must provision
/// it ahead of time as a root-owned, non-symlink directory (normally mode 0700).
/// Each authorization is consumed with O_CREAT|O_EXCL semantics (`create_new`)
/// before the activation command is spawned. A crash can waste authorization,
/// but cannot safely replay it on the next helper invocation.
pub struct FilePrivilegedExecutionLedger {
    root: PathBuf,
}

impl FilePrivilegedExecutionLedger {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn production_default() -> Self {
        Self::new(DEFAULT_EXECUTION_LEDGER_ROOT)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn validate_root(&self) -> Result<(), PrivilegedExecutionLedgerError> {
        let metadata = fs::symlink_metadata(&self.root)
            .map_err(|error| PrivilegedExecutionLedgerError::Io(error.to_string()))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(PrivilegedExecutionLedgerError::InvalidLedgerRoot);
        }

        // Group/other access would let an unprivileged local process interfere
        // with replay protection. Ownership is deployment-specific, but mode is
        // portable enough to enforce at the Linux helper boundary.
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(PrivilegedExecutionLedgerError::InvalidLedgerRoot);
        }
        Ok(())
    }

    fn receipt_path(&self, authorization: &SystemAuthorizationId) -> PathBuf {
        let encoded = authorization
            .as_str()
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        self.root.join(format!("authorization-{encoded}.used"))
    }
}

impl PrivilegedExecutionLedger for FilePrivilegedExecutionLedger {
    fn consume_once(
        &self,
        authorization: &SystemAuthorizationId,
        prepared_at_unix_ms: u64,
    ) -> Result<(), PrivilegedExecutionLedgerError> {
        self.validate_root()?;
        let path = self.receipt_path(authorization);
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(PrivilegedExecutionLedgerError::AlreadyConsumed(
                    authorization.clone(),
                ));
            }
            Err(error) => return Err(PrivilegedExecutionLedgerError::Io(error.to_string())),
        };

        writeln!(file, "authorization={authorization}")
            .and_then(|_| writeln!(file, "prepared-at-unix-ms={prepared_at_unix_ms}"))
            .and_then(|_| file.sync_all())
            .map_err(|error| PrivilegedExecutionLedgerError::Io(error.to_string()))?;

        File::open(&self.root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| PrivilegedExecutionLedgerError::Io(error.to_string()))?;
        Ok(())
    }
}

pub trait NixOsActivationHost {
    fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, String>;
    fn current_system_closure(&self) -> Result<PathBuf, String>;
    fn boot_default_closure(&self) -> Result<PathBuf, String>;
    fn executable_exists(&self, path: &Path) -> Result<bool, String>;
}

pub struct LocalNixOsActivationHost;

impl NixOsActivationHost for LocalNixOsActivationHost {
    fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, String> {
        fs::canonicalize(path).map_err(|error| error.to_string())
    }

    fn current_system_closure(&self) -> Result<PathBuf, String> {
        fs::canonicalize("/run/current-system").map_err(|error| error.to_string())
    }

    fn boot_default_closure(&self) -> Result<PathBuf, String> {
        fs::canonicalize("/nix/var/nix/profiles/system").map_err(|error| error.to_string())
    }

    fn executable_exists(&self, path: &Path) -> Result<bool, String> {
        fs::metadata(path)
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .map_err(|error| error.to_string())
    }
}

pub trait PrivilegedCommandRunner {
    fn run(&self, program: &Path, argument: &str) -> Result<PrivilegedCommandOutcome, String>;
}

pub struct StdPrivilegedCommandRunner;

impl PrivilegedCommandRunner for StdPrivilegedCommandRunner {
    fn run(&self, program: &Path, argument: &str) -> Result<PrivilegedCommandOutcome, String> {
        let started = Instant::now();
        let output = Command::new(program)
            .arg(argument)
            .stdin(Stdio::null())
            .env_clear()
            .env("HOME", "/root")
            .env("USER", "root")
            .env("LOGNAME", "root")
            .env("PATH", "/run/current-system/sw/bin:/run/wrappers/bin")
            .env("LANG", "C")
            .output()
            .map_err(|error| error.to_string())?;
        let duration_us = started.elapsed().as_micros().min(u64::MAX as u128) as u64;

        Ok(PrivilegedCommandOutcome {
            status: if output.status.success() {
                PrivilegedActivationCommandStatus::Succeeded
            } else {
                PrivilegedActivationCommandStatus::Failed
            },
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            duration_us,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrivilegedActivationError {
    NodeMismatch { expected: NodeId, prepared: NodeId },
    TimestampOrderInvalid,
    PreparedActivationStale,
    ReadinessStale,
    AuthorizationExpired,
    UnsupportedAction(SystemCandidateAction),
    AuthorityMismatch,
    EffectClassMismatch,
    InvalidSystemClosure,
    NonCanonicalSystemClosure,
    ProgramMismatch,
    ArgumentsMismatch,
    EvidenceMismatch,
    HostObservation(String),
    CandidateExecutableMissing,
    RollbackExecutableMissing,
    CurrentSystemNotBootDefault,
    ExecutionLedger(PrivilegedExecutionLedgerError),
    Spawn(String),
    UnexpectedLiveSystemChange,
    TestActivationPostconditionFailed,
    RollbackFailed,
}

pub struct PrivilegedNixOsActivationHelper {
    local_node: NodeId,
    policy: PrivilegedActivationRuntimePolicy,
}

impl PrivilegedNixOsActivationHelper {
    pub fn new(local_node: impl Into<NodeId>) -> Self {
        Self {
            local_node: local_node.into(),
            policy: PrivilegedActivationRuntimePolicy::default(),
        }
    }

    pub fn with_policy(
        local_node: impl Into<NodeId>,
        policy: PrivilegedActivationRuntimePolicy,
    ) -> Self {
        Self {
            local_node: local_node.into(),
            policy,
        }
    }

    pub fn validate_prepared(
        &self,
        prepared: &PreparedPrivilegedActivation,
        now_unix_ms: u64,
    ) -> Result<(), PrivilegedActivationError> {
        if prepared.node != self.local_node {
            return Err(PrivilegedActivationError::NodeMismatch {
                expected: self.local_node.clone(),
                prepared: prepared.node.clone(),
            });
        }

        if prepared.readiness_observed_at_unix_ms > prepared.prepared_at_unix_ms
            || prepared.prepared_at_unix_ms > now_unix_ms
            || prepared.authorization_expires_at_unix_ms <= prepared.prepared_at_unix_ms
        {
            return Err(PrivilegedActivationError::TimestampOrderInvalid);
        }
        if now_unix_ms.saturating_sub(prepared.prepared_at_unix_ms)
            > self.policy.max_prepared_age_ms
        {
            return Err(PrivilegedActivationError::PreparedActivationStale);
        }
        if now_unix_ms.saturating_sub(prepared.readiness_observed_at_unix_ms)
            > self.policy.max_readiness_age_ms
        {
            return Err(PrivilegedActivationError::ReadinessStale);
        }
        if now_unix_ms >= prepared.authorization_expires_at_unix_ms {
            return Err(PrivilegedActivationError::AuthorizationExpired);
        }

        let (expected_effect, expected_argument) = match prepared.plan.action {
            SystemCandidateAction::PreviewActivation => (SystemEffectClass::PreviewHooks, "dry-activate"),
            SystemCandidateAction::TestActivation => {
                (SystemEffectClass::TemporaryLiveActivation, "test")
            }
            ref other => {
                return Err(PrivilegedActivationError::UnsupportedAction(other.clone()));
            }
        };

        if prepared.plan.authority != SystemAuthorityClass::HostAdministrator {
            return Err(PrivilegedActivationError::AuthorityMismatch);
        }
        if prepared.plan.effect_class != expected_effect {
            return Err(PrivilegedActivationError::EffectClassMismatch);
        }
        if !valid_store_closure(Path::new(&prepared.plan.system_closure)) {
            return Err(PrivilegedActivationError::InvalidSystemClosure);
        }

        let expected_program = format!(
            "{}/bin/switch-to-configuration",
            prepared.plan.system_closure
        );
        if prepared.plan.program != expected_program {
            return Err(PrivilegedActivationError::ProgramMismatch);
        }
        if prepared.plan.args.as_slice() != [expected_argument] {
            return Err(PrivilegedActivationError::ArgumentsMismatch);
        }

        let authorization_evidence = format!("authorization:{}", prepared.authorization);
        let expiry_evidence = format!(
            "expires-at-unix-ms:{}",
            prepared.authorization_expires_at_unix_ms
        );
        let node_evidence = format!("node:{}", prepared.node);
        let readiness_evidence = format!(
            "observed-at-unix-ms:{}",
            prepared.readiness_observed_at_unix_ms
        );
        if !prepared.authorization_evidence.contains(&authorization_evidence)
            || !prepared.authorization_evidence.contains(&expiry_evidence)
            || !prepared.readiness_evidence.contains(&node_evidence)
            || !prepared.readiness_evidence.contains(&readiness_evidence)
        {
            return Err(PrivilegedActivationError::EvidenceMismatch);
        }

        Ok(())
    }

    pub fn execute<H, R, L>(
        &self,
        prepared: &PreparedPrivilegedActivation,
        now_unix_ms: u64,
        host: &H,
        runner: &R,
        ledger: &L,
    ) -> Result<PrivilegedActivationExecution, PrivilegedActivationError>
    where
        H: NixOsActivationHost,
        R: PrivilegedCommandRunner,
        L: PrivilegedExecutionLedger,
    {
        self.validate_prepared(prepared, now_unix_ms)?;

        let candidate = PathBuf::from(&prepared.plan.system_closure);
        let canonical_candidate = host
            .canonicalize_path(&candidate)
            .map_err(PrivilegedActivationError::HostObservation)?;
        if canonical_candidate != candidate {
            return Err(PrivilegedActivationError::NonCanonicalSystemClosure);
        }

        let candidate_program = PathBuf::from(&prepared.plan.program);
        if !host
            .executable_exists(&candidate_program)
            .map_err(PrivilegedActivationError::HostObservation)?
        {
            return Err(PrivilegedActivationError::CandidateExecutableMissing);
        }

        let before = host
            .current_system_closure()
            .map_err(PrivilegedActivationError::HostObservation)?;
        let boot_default = host
            .boot_default_closure()
            .map_err(PrivilegedActivationError::HostObservation)?;
        if !valid_store_closure(&before) || !valid_store_closure(&boot_default) {
            return Err(PrivilegedActivationError::InvalidSystemClosure);
        }
        if before != boot_default {
            return Err(PrivilegedActivationError::CurrentSystemNotBootDefault);
        }

        let rollback_program = before.join("bin/switch-to-configuration");
        if !host
            .executable_exists(&rollback_program)
            .map_err(PrivilegedActivationError::HostObservation)?
        {
            return Err(PrivilegedActivationError::RollbackExecutableMissing);
        }

        // The privileged process owns a second, durable replay barrier. The
        // unprivileged gate may have consumed the same receipt in its own
        // ledger, but that cannot protect a separately-invoked root helper.
        ledger
            .consume_once(&prepared.authorization, prepared.prepared_at_unix_ms)
            .map_err(PrivilegedActivationError::ExecutionLedger)?;

        let argument = prepared.plan.args[0].as_str();
        let command = runner
            .run(&candidate_program, argument)
            .map_err(PrivilegedActivationError::Spawn)?;
        let mut after = host
            .current_system_closure()
            .map_err(PrivilegedActivationError::HostObservation)?;
        if !valid_store_closure(&after) {
            return Err(PrivilegedActivationError::InvalidSystemClosure);
        }

        match prepared.plan.action {
            SystemCandidateAction::PreviewActivation => {
                if after != before {
                    self.restore_baseline(&before, &rollback_program, host, runner)?;
                    return Err(PrivilegedActivationError::UnexpectedLiveSystemChange);
                }

                Ok(PrivilegedActivationExecution {
                    node: prepared.node.clone(),
                    authorization: prepared.authorization.clone(),
                    action: prepared.plan.action.clone(),
                    before_system_closure: path_text(&before),
                    after_system_closure: path_text(&after),
                    command,
                    rollback_attempted: false,
                    rollback_succeeded: false,
                })
            }
            SystemCandidateAction::TestActivation => {
                if command.succeeded() {
                    if after != candidate {
                        self.restore_baseline(&before, &rollback_program, host, runner)?;
                        return Err(PrivilegedActivationError::TestActivationPostconditionFailed);
                    }

                    return Ok(PrivilegedActivationExecution {
                        node: prepared.node.clone(),
                        authorization: prepared.authorization.clone(),
                        action: prepared.plan.action.clone(),
                        before_system_closure: path_text(&before),
                        after_system_closure: path_text(&after),
                        command,
                        rollback_attempted: false,
                        rollback_succeeded: false,
                    });
                }

                // A failed temporary activation is treated as an interrupted
                // state transition. Re-activate the exact pre-operation closure
                // even if /run/current-system still points at it: services may
                // have been partially stopped/restarted before failure.
                self.restore_baseline(&before, &rollback_program, host, runner)?;
                after = host
                    .current_system_closure()
                    .map_err(PrivilegedActivationError::HostObservation)?;

                Ok(PrivilegedActivationExecution {
                    node: prepared.node.clone(),
                    authorization: prepared.authorization.clone(),
                    action: prepared.plan.action.clone(),
                    before_system_closure: path_text(&before),
                    after_system_closure: path_text(&after),
                    command,
                    rollback_attempted: true,
                    rollback_succeeded: true,
                })
            }
            _ => unreachable!("validate_prepared restricts the privileged action set"),
        }
    }

    fn restore_baseline<H, R>(
        &self,
        baseline: &Path,
        rollback_program: &Path,
        host: &H,
        runner: &R,
    ) -> Result<(), PrivilegedActivationError>
    where
        H: NixOsActivationHost,
        R: PrivilegedCommandRunner,
    {
        let rollback = runner
            .run(rollback_program, "test")
            .map_err(PrivilegedActivationError::Spawn)?;
        if !rollback.succeeded() {
            return Err(PrivilegedActivationError::RollbackFailed);
        }
        let restored = host
            .current_system_closure()
            .map_err(PrivilegedActivationError::HostObservation)?;
        if restored != baseline {
            return Err(PrivilegedActivationError::RollbackFailed);
        }
        Ok(())
    }
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn valid_store_closure(path: &Path) -> bool {
    let components = path.components().collect::<Vec<_>>();
    matches!(
        components.as_slice(),
        [
            Component::RootDir,
            Component::Normal(nix),
            Component::Normal(store),
            Component::Normal(closure)
        ] if *nix == "nix" && *store == "store" && !closure.is_empty()
    )
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use blob_core::{
        SystemAuthorityClass, SystemCandidateId, SystemEffectClass, SystemOperationId, SystemSpecId,
    };
    use blob_nix_nixos_activation::ImmutableNixOsActivationPlan;

    use super::*;

    const BASELINE: &str = "/nix/store/baseline-nixos-system-blob-pilot";
    const CANDIDATE: &str = "/nix/store/candidate-nixos-system-blob-pilot";

    fn prepared(action: SystemCandidateAction) -> PreparedPrivilegedActivation {
        let (effect_class, argument) = match action {
            SystemCandidateAction::PreviewActivation => (SystemEffectClass::PreviewHooks, "dry-activate"),
            SystemCandidateAction::TestActivation => {
                (SystemEffectClass::TemporaryLiveActivation, "test")
            }
            _ => unreachable!(),
        };
        PreparedPrivilegedActivation {
            node: NodeId::from("node:lab"),
            readiness_observed_at_unix_ms: 1_000,
            authorization: SystemAuthorizationId::from("auth:one"),
            authorization_expires_at_unix_ms: 61_000,
            prepared_at_unix_ms: 2_000,
            plan: ImmutableNixOsActivationPlan {
                operation_id: SystemOperationId::from("op:activate"),
                candidate: SystemCandidateId::from("candidate:one"),
                system_spec: SystemSpecId::from("system:one"),
                materialization_operation: SystemOperationId::from("op:materialize"),
                system_closure: CANDIDATE.into(),
                action: action.clone(),
                effect_class,
                authority: SystemAuthorityClass::HostAdministrator,
                program: format!("{CANDIDATE}/bin/switch-to-configuration"),
                args: vec![argument.into()],
                expected_effects: vec![],
                rollback_semantics: "reboot restores baseline".into(),
            },
            readiness_evidence: vec![
                "node:node:lab".into(),
                "observed-at-unix-ms:1000".into(),
            ],
            authorization_evidence: vec![
                "authorization:auth:one".into(),
                "expires-at-unix-ms:61000".into(),
            ],
        }
    }

    #[derive(Debug)]
    struct FakeState {
        current: PathBuf,
        boot_default: PathBuf,
        fail_candidate: bool,
        fail_rollback: bool,
        candidate_failure_changes_live_state: bool,
        calls: Vec<(PathBuf, String)>,
    }

    fn fake_state() -> Arc<Mutex<FakeState>> {
        Arc::new(Mutex::new(FakeState {
            current: PathBuf::from(BASELINE),
            boot_default: PathBuf::from(BASELINE),
            fail_candidate: false,
            fail_rollback: false,
            candidate_failure_changes_live_state: false,
            calls: Vec::new(),
        }))
    }

    struct FakeHost {
        state: Arc<Mutex<FakeState>>,
    }

    impl NixOsActivationHost for FakeHost {
        fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, String> {
            Ok(path.to_path_buf())
        }

        fn current_system_closure(&self) -> Result<PathBuf, String> {
            self.state
                .lock()
                .map(|state| state.current.clone())
                .map_err(|_| "state poisoned".into())
        }

        fn boot_default_closure(&self) -> Result<PathBuf, String> {
            self.state
                .lock()
                .map(|state| state.boot_default.clone())
                .map_err(|_| "state poisoned".into())
        }

        fn executable_exists(&self, path: &Path) -> Result<bool, String> {
            Ok(path == Path::new(&format!("{CANDIDATE}/bin/switch-to-configuration"))
                || path == Path::new(&format!("{BASELINE}/bin/switch-to-configuration")))
        }
    }

    struct FakeRunner {
        state: Arc<Mutex<FakeState>>,
    }

    impl PrivilegedCommandRunner for FakeRunner {
        fn run(&self, program: &Path, argument: &str) -> Result<PrivilegedCommandOutcome, String> {
            let mut state = self.state.lock().map_err(|_| "state poisoned".to_string())?;
            state.calls.push((program.to_path_buf(), argument.into()));

            let is_candidate = program == Path::new(&format!("{CANDIDATE}/bin/switch-to-configuration"));
            let is_rollback = program == Path::new(&format!("{BASELINE}/bin/switch-to-configuration"));

            if is_candidate && argument == "test" {
                if state.fail_candidate {
                    if state.candidate_failure_changes_live_state {
                        state.current = PathBuf::from(CANDIDATE);
                    }
                    return Ok(failed_outcome("candidate failed"));
                }
                state.current = PathBuf::from(CANDIDATE);
            }
            if is_rollback && argument == "test" {
                if state.fail_rollback {
                    return Ok(failed_outcome("rollback failed"));
                }
                state.current = PathBuf::from(BASELINE);
            }

            Ok(success_outcome())
        }
    }

    fn success_outcome() -> PrivilegedCommandOutcome {
        PrivilegedCommandOutcome {
            status: PrivilegedActivationCommandStatus::Succeeded,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            duration_us: 1,
        }
    }

    fn failed_outcome(stderr: &str) -> PrivilegedCommandOutcome {
        PrivilegedCommandOutcome {
            status: PrivilegedActivationCommandStatus::Failed,
            exit_code: Some(1),
            stdout: String::new(),
            stderr: stderr.into(),
            duration_us: 1,
        }
    }

    fn fixture() -> (
        Arc<Mutex<FakeState>>,
        FakeHost,
        FakeRunner,
        InMemoryPrivilegedExecutionLedger,
    ) {
        let state = fake_state();
        (
            Arc::clone(&state),
            FakeHost {
                state: Arc::clone(&state),
            },
            FakeRunner {
                state: Arc::clone(&state),
            },
            InMemoryPrivilegedExecutionLedger::default(),
        )
    }

    #[test]
    fn preview_executes_only_dry_activate_and_keeps_baseline() {
        let (state, host, runner, ledger) = fixture();
        let request = prepared(SystemCandidateAction::PreviewActivation);
        let result = PrivilegedNixOsActivationHelper::new("node:lab")
            .execute(&request, 2_100, &host, &runner, &ledger)
            .expect("preview must pass");

        assert!(result.command.succeeded());
        assert_eq!(result.before_system_closure, BASELINE);
        assert_eq!(result.after_system_closure, BASELINE);
        assert!(!result.rollback_attempted);
        assert!(ledger.was_consumed(&request.authorization));
        assert_eq!(state.lock().unwrap().calls[0].1, "dry-activate");
    }

    #[test]
    fn test_activation_switches_to_exact_candidate_without_changing_boot_default() {
        let (state, host, runner, ledger) = fixture();
        let request = prepared(SystemCandidateAction::TestActivation);
        let result = PrivilegedNixOsActivationHelper::new("node:lab")
            .execute(&request, 2_100, &host, &runner, &ledger)
            .expect("test activation must pass");

        assert!(result.command.succeeded());
        assert_eq!(result.after_system_closure, CANDIDATE);
        let state = state.lock().unwrap();
        assert_eq!(state.current, PathBuf::from(CANDIDATE));
        assert_eq!(state.boot_default, PathBuf::from(BASELINE));
        assert_eq!(state.calls.len(), 1);
        assert_eq!(state.calls[0].1, "test");
    }

    #[test]
    fn prepared_action_is_bound_to_local_node() {
        let (_, host, runner, ledger) = fixture();
        let request = prepared(SystemCandidateAction::TestActivation);
        assert!(matches!(
            PrivilegedNixOsActivationHelper::new("node:other")
                .execute(&request, 2_100, &host, &runner, &ledger),
            Err(PrivilegedActivationError::NodeMismatch { .. })
        ));
        assert!(!ledger.was_consumed(&request.authorization));
    }

    #[test]
    fn stale_prepared_action_is_rejected_before_replay_ledger_consumption() {
        let (_, host, runner, ledger) = fixture();
        let request = prepared(SystemCandidateAction::TestActivation);
        assert_eq!(
            PrivilegedNixOsActivationHelper::new("node:lab")
                .execute(&request, 40_001, &host, &runner, &ledger),
            Err(PrivilegedActivationError::PreparedActivationStale)
        );
        assert!(!ledger.was_consumed(&request.authorization));
    }

    #[test]
    fn authorization_expiry_is_rechecked_at_privileged_boundary() {
        let (_, host, runner, ledger) = fixture();
        let request = prepared(SystemCandidateAction::TestActivation);
        let helper = PrivilegedNixOsActivationHelper::with_policy(
            "node:lab",
            PrivilegedActivationRuntimePolicy {
                max_prepared_age_ms: u64::MAX,
                max_readiness_age_ms: u64::MAX,
            },
        );
        assert_eq!(
            helper.execute(&request, 61_000, &host, &runner, &ledger),
            Err(PrivilegedActivationError::AuthorizationExpired)
        );
    }

    #[test]
    fn forged_program_is_rejected() {
        let (_, host, runner, ledger) = fixture();
        let mut request = prepared(SystemCandidateAction::TestActivation);
        request.plan.program = "/bin/sh".into();
        assert_eq!(
            PrivilegedNixOsActivationHelper::new("node:lab")
                .execute(&request, 2_100, &host, &runner, &ledger),
            Err(PrivilegedActivationError::ProgramMismatch)
        );
    }

    #[test]
    fn persistent_switch_and_boot_arguments_are_impossible_at_helper_boundary() {
        let (_, host, runner, ledger) = fixture();
        for forbidden in ["switch", "boot"] {
            let mut request = prepared(SystemCandidateAction::TestActivation);
            request.plan.args = vec![forbidden.into()];
            assert_eq!(
                PrivilegedNixOsActivationHelper::new("node:lab")
                    .execute(&request, 2_100, &host, &runner, &ledger),
                Err(PrivilegedActivationError::ArgumentsMismatch)
            );
        }
    }

    #[test]
    fn a_prepared_authorization_cannot_execute_twice() {
        let (_, host, runner, ledger) = fixture();
        let request = prepared(SystemCandidateAction::PreviewActivation);
        let helper = PrivilegedNixOsActivationHelper::new("node:lab");
        helper
            .execute(&request, 2_100, &host, &runner, &ledger)
            .expect("first use succeeds");
        assert!(matches!(
            helper.execute(&request, 2_101, &host, &runner, &ledger),
            Err(PrivilegedActivationError::ExecutionLedger(
                PrivilegedExecutionLedgerError::AlreadyConsumed(_)
            ))
        ));
    }

    #[test]
    fn helper_rejects_stacked_temporary_activations() {
        let (state, host, runner, ledger) = fixture();
        state.lock().unwrap().current = PathBuf::from(CANDIDATE);
        let request = prepared(SystemCandidateAction::TestActivation);
        assert_eq!(
            PrivilegedNixOsActivationHelper::new("node:lab")
                .execute(&request, 2_100, &host, &runner, &ledger),
            Err(PrivilegedActivationError::CurrentSystemNotBootDefault)
        );
        assert!(!ledger.was_consumed(&request.authorization));
    }

    #[test]
    fn failed_test_activation_reapplies_exact_baseline() {
        let (state, host, runner, ledger) = fixture();
        {
            let mut state = state.lock().unwrap();
            state.fail_candidate = true;
            state.candidate_failure_changes_live_state = true;
        }
        let request = prepared(SystemCandidateAction::TestActivation);
        let result = PrivilegedNixOsActivationHelper::new("node:lab")
            .execute(&request, 2_100, &host, &runner, &ledger)
            .expect("successful rollback returns an execution report");

        assert!(!result.command.succeeded());
        assert!(result.rollback_attempted);
        assert!(result.rollback_succeeded);
        assert_eq!(result.after_system_closure, BASELINE);
        let state = state.lock().unwrap();
        assert_eq!(state.current, PathBuf::from(BASELINE));
        assert_eq!(state.calls.len(), 2);
        assert_eq!(state.calls[1].0, PathBuf::from(format!("{BASELINE}/bin/switch-to-configuration")));
        assert_eq!(state.calls[1].1, "test");
    }

    #[test]
    fn rollback_failure_is_fail_closed() {
        let (state, host, runner, ledger) = fixture();
        {
            let mut state = state.lock().unwrap();
            state.fail_candidate = true;
            state.fail_rollback = true;
            state.candidate_failure_changes_live_state = true;
        }
        let request = prepared(SystemCandidateAction::TestActivation);
        assert_eq!(
            PrivilegedNixOsActivationHelper::new("node:lab")
                .execute(&request, 2_100, &host, &runner, &ledger),
            Err(PrivilegedActivationError::RollbackFailed)
        );
    }

    #[test]
    fn file_ledger_uses_create_new_for_cross_process_replay_protection() {
        let unique = format!(
            "blob-privileged-ledger-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let ledger = FilePrivilegedExecutionLedger::new(&root);
        let id = SystemAuthorizationId::from("auth:persistent");

        ledger.consume_once(&id, 1_000).expect("first use");
        assert!(matches!(
            ledger.consume_once(&id, 1_001),
            Err(PrivilegedExecutionLedgerError::AlreadyConsumed(_))
        ));

        fs::remove_dir_all(root).unwrap();
    }
}
