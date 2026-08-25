#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use blob_core::{
    NodeId, SystemAuthorityClass, SystemAuthorizationId, SystemCandidateAction, SystemCandidateId,
    SystemEffectClass, SystemOperationId, SystemSpecId,
};
use blob_nix_nixos_privileged_helper::{
    NixOsActivationHost, PrivilegedActivationError, PrivilegedActivationExecution,
    PrivilegedActivationRuntimePolicy, PrivilegedCommandRunner, PrivilegedExecutionLedger,
    PrivilegedNixOsActivationHelper,
};
use blob_system_activation_gate::PreparedPrivilegedActivation;

pub const DEFAULT_TRUSTED_PERMIT_ROOT: &str = "/var/lib/theblob/activation-permits";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedActivationPermit {
    pub authorization: SystemAuthorizationId,
    pub node: NodeId,
    pub operation: SystemOperationId,
    pub candidate: SystemCandidateId,
    pub system_spec: SystemSpecId,
    pub materialization_operation: SystemOperationId,
    pub action: SystemCandidateAction,
    pub effect_class: SystemEffectClass,
    pub authority: SystemAuthorityClass,
    pub system_closure: String,
    pub program: String,
    pub args: Vec<String>,
    pub readiness_observed_at_unix_ms: u64,
    pub prepared_at_unix_ms: u64,
    pub authorization_expires_at_unix_ms: u64,
}

impl TrustedActivationPermit {
    pub fn from_prepared(prepared: &PreparedPrivilegedActivation) -> Self {
        Self {
            authorization: prepared.authorization.clone(),
            node: prepared.node.clone(),
            operation: prepared.plan.operation_id.clone(),
            candidate: prepared.plan.candidate.clone(),
            system_spec: prepared.plan.system_spec.clone(),
            materialization_operation: prepared.plan.materialization_operation.clone(),
            action: prepared.plan.action.clone(),
            effect_class: prepared.plan.effect_class.clone(),
            authority: prepared.plan.authority.clone(),
            system_closure: prepared.plan.system_closure.clone(),
            program: prepared.plan.program.clone(),
            args: prepared.plan.args.clone(),
            readiness_observed_at_unix_ms: prepared.readiness_observed_at_unix_ms,
            prepared_at_unix_ms: prepared.prepared_at_unix_ms,
            authorization_expires_at_unix_ms: prepared.authorization_expires_at_unix_ms,
        }
    }

    /// Canonical, injection-safe representation stored by the privileged issuer.
    ///
    /// All user-controlled text is hex encoded, so identifiers cannot inject
    /// extra fields. The format is deliberately versioned and exact: any future
    /// incompatible representation must use a new version string.
    pub fn canonical_text(&self) -> String {
        let mut lines = vec![
            "theblob-activation-permit-v1".to_owned(),
            format!("authorization={}", hex_text(self.authorization.as_str())),
            format!("node={}", hex_text(self.node.as_str())),
            format!("operation={}", hex_text(self.operation.as_str())),
            format!("candidate={}", hex_text(self.candidate.as_str())),
            format!("system-spec={}", hex_text(self.system_spec.as_str())),
            format!(
                "materialization-operation={}",
                hex_text(self.materialization_operation.as_str())
            ),
            format!("action={}", action_token(&self.action)),
            format!("effect-class={}", effect_token(&self.effect_class)),
            format!("authority={}", authority_token(&self.authority)),
            format!("system-closure={}", hex_text(&self.system_closure)),
            format!("program={}", hex_text(&self.program)),
            format!("args-count={}", self.args.len()),
        ];
        lines.extend(
            self.args
                .iter()
                .enumerate()
                .map(|(index, arg)| format!("arg-{index}={}", hex_text(arg))),
        );
        lines.extend([
            format!(
                "readiness-observed-at-unix-ms={}",
                self.readiness_observed_at_unix_ms
            ),
            format!("prepared-at-unix-ms={}", self.prepared_at_unix_ms),
            format!(
                "authorization-expires-at-unix-ms={}",
                self.authorization_expires_at_unix_ms
            ),
        ]);
        lines.push(String::new());
        lines.join("\n")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrustedActivationPermitError {
    MissingOrAlreadyConsumed(SystemAuthorizationId),
    PermitMismatch,
    InvalidPermitRoot,
    InvalidPermitFile,
    Io(String),
}

pub trait TrustedActivationPermitStore {
    /// Authenticate and destructively consume the exact capability for `prepared`.
    ///
    /// A consumed permit is never restored if a later host check or activation
    /// fails. Losing liveness is safer than making privileged authority replayable.
    fn consume_matching(
        &self,
        prepared: &PreparedPrivilegedActivation,
    ) -> Result<(), TrustedActivationPermitError>;
}

/// Non-production permit store for deterministic composition tests.
#[derive(Default)]
pub struct InMemoryTrustedActivationPermitStore {
    permits: Mutex<BTreeSet<String>>,
}

impl InMemoryTrustedActivationPermitStore {
    pub fn grant(&self, permit: TrustedActivationPermit) {
        self.permits
            .lock()
            .expect("in-memory trusted permit store poisoned")
            .insert(permit.canonical_text());
    }

    pub fn grant_for(&self, prepared: &PreparedPrivilegedActivation) {
        self.grant(TrustedActivationPermit::from_prepared(prepared));
    }
}

impl TrustedActivationPermitStore for InMemoryTrustedActivationPermitStore {
    fn consume_matching(
        &self,
        prepared: &PreparedPrivilegedActivation,
    ) -> Result<(), TrustedActivationPermitError> {
        let expected = TrustedActivationPermit::from_prepared(prepared).canonical_text();
        let mut permits = self
            .permits
            .lock()
            .map_err(|_| TrustedActivationPermitError::Io("permit store poisoned".into()))?;
        if permits.remove(&expected) {
            Ok(())
        } else {
            Err(TrustedActivationPermitError::MissingOrAlreadyConsumed(
                prepared.authorization.clone(),
            ))
        }
    }
}

/// Read/consume side of the production trusted permit store.
///
/// The directory and permit files must already have been created by a future
/// OS-authenticated privileged issuer. This type intentionally exposes no API to
/// mint production permits. For the production default, both directory and file
/// owner must be root; the directory must be inaccessible to group/other and a
/// permit must be an ordinary mode-0600 file, never a symlink.
pub struct FileTrustedActivationPermitStore {
    root: PathBuf,
    expected_owner_uid: u32,
}

impl FileTrustedActivationPermitStore {
    pub fn production_default() -> Self {
        Self::new(DEFAULT_TRUSTED_PERMIT_ROOT, 0)
    }

    pub fn new(root: impl Into<PathBuf>, expected_owner_uid: u32) -> Self {
        Self {
            root: root.into(),
            expected_owner_uid,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn validate_root(&self) -> Result<(), TrustedActivationPermitError> {
        let metadata = fs::symlink_metadata(&self.root)
            .map_err(|error| TrustedActivationPermitError::Io(error.to_string()))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.uid() != self.expected_owner_uid
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(TrustedActivationPermitError::InvalidPermitRoot);
        }
        Ok(())
    }

    fn permit_path(&self, authorization: &SystemAuthorizationId) -> PathBuf {
        self.root.join(format!(
            "authorization-{}.permit",
            hex_text(authorization.as_str())
        ))
    }

    fn validate_file(
        &self,
        path: &Path,
        authorization: &SystemAuthorizationId,
    ) -> Result<(), TrustedActivationPermitError> {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                TrustedActivationPermitError::MissingOrAlreadyConsumed(authorization.clone())
            } else {
                TrustedActivationPermitError::Io(error.to_string())
            }
        })?;

        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != self.expected_owner_uid
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(TrustedActivationPermitError::InvalidPermitFile);
        }
        Ok(())
    }
}

impl TrustedActivationPermitStore for FileTrustedActivationPermitStore {
    fn consume_matching(
        &self,
        prepared: &PreparedPrivilegedActivation,
    ) -> Result<(), TrustedActivationPermitError> {
        self.validate_root()?;
        let path = self.permit_path(&prepared.authorization);
        self.validate_file(&path, &prepared.authorization)?;

        let mut observed = String::new();
        File::open(&path)
            .and_then(|mut file| file.read_to_string(&mut observed))
            .map_err(|error| TrustedActivationPermitError::Io(error.to_string()))?;

        let expected = TrustedActivationPermit::from_prepared(prepared).canonical_text();
        if observed != expected {
            return Err(TrustedActivationPermitError::PermitMismatch);
        }

        // The root-owned directory is not writable by the unprivileged caller.
        // Concurrent helpers may both read, but only one can win the destructive
        // remove; the loser observes NotFound and fails closed before execution.
        fs::remove_file(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                TrustedActivationPermitError::MissingOrAlreadyConsumed(
                    prepared.authorization.clone(),
                )
            } else {
                TrustedActivationPermitError::Io(error.to_string())
            }
        })?;
        File::open(&self.root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| TrustedActivationPermitError::Io(error.to_string()))?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootActivationBoundaryError {
    PreparedRejected(PrivilegedActivationError),
    TrustedPermitRejected(TrustedActivationPermitError),
    RuntimeRejected(PrivilegedActivationError),
}

/// Intended public entry point for any future root service or privileged IPC.
///
/// The lower-level helper remains a mechanism crate. This boundary adds the
/// missing proof of authority: an exact root-owned capability must already exist
/// and is destroyed before the mechanism is allowed to attempt activation.
pub struct RootOwnedNixOsActivationBoundary {
    runtime: PrivilegedNixOsActivationHelper,
}

impl RootOwnedNixOsActivationBoundary {
    pub fn new(local_node: impl Into<NodeId>) -> Self {
        Self {
            runtime: PrivilegedNixOsActivationHelper::new(local_node),
        }
    }

    pub fn with_policy(
        local_node: impl Into<NodeId>,
        policy: PrivilegedActivationRuntimePolicy,
    ) -> Self {
        Self {
            runtime: PrivilegedNixOsActivationHelper::with_policy(local_node, policy),
        }
    }

    pub fn execute<H, R, P, L>(
        &self,
        prepared: &PreparedPrivilegedActivation,
        now_unix_ms: u64,
        host: &H,
        runner: &R,
        permits: &P,
        replay_ledger: &L,
    ) -> Result<PrivilegedActivationExecution, RootActivationBoundaryError>
    where
        H: NixOsActivationHost,
        R: PrivilegedCommandRunner,
        P: TrustedActivationPermitStore,
        L: PrivilegedExecutionLedger,
    {
        // Reject malformed/stale/future/expired requests before spending the
        // trusted capability. The mechanism repeats this validation internally.
        self.runtime
            .validate_prepared(prepared, now_unix_ms)
            .map_err(RootActivationBoundaryError::PreparedRejected)?;

        // This is the authority-crossing point. Successful consumption means a
        // privileged issuer previously granted exactly this node/operation/
        // candidate/SystemSpec/action/closure/timestamp tuple.
        permits
            .consume_matching(prepared)
            .map_err(RootActivationBoundaryError::TrustedPermitRejected)?;

        self.runtime
            .execute(prepared, now_unix_ms, host, runner, replay_ledger)
            .map_err(RootActivationBoundaryError::RuntimeRejected)
    }
}

fn hex_text(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn action_token(action: &SystemCandidateAction) -> &'static str {
    match action {
        SystemCandidateAction::Materialize => "materialize",
        SystemCandidateAction::PreviewActivation => "preview-activation",
        SystemCandidateAction::TestActivation => "test-activation",
        SystemCandidateAction::BuildIsolatedVm => "build-isolated-vm",
    }
}

fn effect_token(effect: &SystemEffectClass) -> &'static str {
    match effect {
        SystemEffectClass::MaterializationOnly => "materialization-only",
        SystemEffectClass::PreviewHooks => "preview-hooks",
        SystemEffectClass::TemporaryLiveActivation => "temporary-live-activation",
    }
}

fn authority_token(authority: &SystemAuthorityClass) -> &'static str {
    match authority {
        SystemAuthorityClass::User => "user",
        SystemAuthorityClass::HostAdministrator => "host-administrator",
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::sync::{Arc, Mutex};

    use blob_nix_nixos_activation::ImmutableNixOsActivationPlan;
    use blob_nix_nixos_privileged_helper::{
        InMemoryPrivilegedExecutionLedger, PrivilegedActivationCommandStatus,
        PrivilegedCommandOutcome,
    };

    use super::*;

    const BASELINE: &str = "/nix/store/baseline-nixos-system-blob-pilot";
    const CANDIDATE: &str = "/nix/store/candidate-nixos-system-blob-pilot";

    fn prepared(action: SystemCandidateAction) -> PreparedPrivilegedActivation {
        let (effect_class, argument) = match action {
            SystemCandidateAction::PreviewActivation => {
                (SystemEffectClass::PreviewHooks, "dry-activate")
            }
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
        calls: Vec<(PathBuf, String)>,
    }

    fn fake_state() -> Arc<Mutex<FakeState>> {
        Arc::new(Mutex::new(FakeState {
            current: PathBuf::from(BASELINE),
            boot_default: PathBuf::from(BASELINE),
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
            if program == Path::new(&format!("{CANDIDATE}/bin/switch-to-configuration"))
                && argument == "test"
            {
                state.current = PathBuf::from(CANDIDATE);
            }
            Ok(PrivilegedCommandOutcome {
                status: PrivilegedActivationCommandStatus::Succeeded,
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
                duration_us: 1,
            })
        }
    }

    fn fixture() -> (
        Arc<Mutex<FakeState>>,
        FakeHost,
        FakeRunner,
        InMemoryTrustedActivationPermitStore,
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
            InMemoryTrustedActivationPermitStore::default(),
            InMemoryPrivilegedExecutionLedger::default(),
        )
    }

    #[test]
    fn no_trusted_permit_means_no_privileged_command() {
        let request = prepared(SystemCandidateAction::PreviewActivation);
        let (state, host, runner, permits, replay) = fixture();

        assert!(matches!(
            RootOwnedNixOsActivationBoundary::new("node:lab").execute(
                &request, 2_100, &host, &runner, &permits, &replay
            ),
            Err(RootActivationBoundaryError::TrustedPermitRejected(
                TrustedActivationPermitError::MissingOrAlreadyConsumed(_)
            ))
        ));
        assert!(state.lock().unwrap().calls.is_empty());
        assert!(!replay.was_consumed(&request.authorization));
    }

    #[test]
    fn exact_trusted_permit_allows_preview_once() {
        let request = prepared(SystemCandidateAction::PreviewActivation);
        let (state, host, runner, permits, replay) = fixture();
        permits.grant_for(&request);
        let boundary = RootOwnedNixOsActivationBoundary::new("node:lab");

        let result = boundary
            .execute(&request, 2_100, &host, &runner, &permits, &replay)
            .expect("exact trusted permit should authorize preview");
        assert_eq!(result.after_system_closure, BASELINE);
        assert_eq!(state.lock().unwrap().calls.len(), 1);

        assert!(matches!(
            boundary.execute(&request, 2_101, &host, &runner, &permits, &replay),
            Err(RootActivationBoundaryError::TrustedPermitRejected(
                TrustedActivationPermitError::MissingOrAlreadyConsumed(_)
            ))
        ));
        assert_eq!(state.lock().unwrap().calls.len(), 1);
    }

    #[test]
    fn exact_trusted_permit_allows_test_activation_of_exact_closure() {
        let request = prepared(SystemCandidateAction::TestActivation);
        let (state, host, runner, permits, replay) = fixture();
        permits.grant_for(&request);

        let result = RootOwnedNixOsActivationBoundary::new("node:lab")
            .execute(&request, 2_100, &host, &runner, &permits, &replay)
            .expect("exact trusted permit should authorize test activation");
        assert_eq!(result.after_system_closure, CANDIDATE);
        let state = state.lock().unwrap();
        assert_eq!(state.current, PathBuf::from(CANDIDATE));
        assert_eq!(state.boot_default, PathBuf::from(BASELINE));
    }

    #[test]
    fn internally_valid_but_forged_candidate_id_is_denied_by_permit() {
        let real = prepared(SystemCandidateAction::PreviewActivation);
        let mut forged = real.clone();
        forged.plan.candidate = SystemCandidateId::from("candidate:forged");
        let (state, host, runner, permits, replay) = fixture();
        permits.grant_for(&real);

        assert!(matches!(
            RootOwnedNixOsActivationBoundary::new("node:lab").execute(
                &forged, 2_100, &host, &runner, &permits, &replay
            ),
            Err(RootActivationBoundaryError::TrustedPermitRejected(_))
        ));
        assert!(state.lock().unwrap().calls.is_empty());
        permits
            .consume_matching(&real)
            .expect("forged request must not consume the real permit");
    }

    #[test]
    fn stale_request_is_rejected_before_spending_trusted_permit() {
        let request = prepared(SystemCandidateAction::PreviewActivation);
        let (state, host, runner, permits, replay) = fixture();
        permits.grant_for(&request);

        assert!(matches!(
            RootOwnedNixOsActivationBoundary::new("node:lab").execute(
                &request, 40_001, &host, &runner, &permits, &replay
            ),
            Err(RootActivationBoundaryError::PreparedRejected(
                PrivilegedActivationError::PreparedActivationStale
            ))
        ));
        assert!(state.lock().unwrap().calls.is_empty());
        permits
            .consume_matching(&request)
            .expect("pre-validation rejection must not spend permit");
    }

    #[test]
    fn canonical_permit_binds_all_execution_sensitive_fields() {
        let baseline = TrustedActivationPermit::from_prepared(&prepared(
            SystemCandidateAction::TestActivation,
        ))
        .canonical_text();

        let mutations: Vec<Box<dyn Fn(&mut PreparedPrivilegedActivation)>> = vec![
            Box::new(|p| p.node = NodeId::from("node:other")),
            Box::new(|p| p.plan.operation_id = SystemOperationId::from("op:other")),
            Box::new(|p| p.plan.candidate = SystemCandidateId::from("candidate:other")),
            Box::new(|p| p.plan.system_spec = SystemSpecId::from("system:other")),
            Box::new(|p| {
                p.plan.materialization_operation =
                    SystemOperationId::from("op:other-materialize")
            }),
            Box::new(|p| p.plan.system_closure = "/nix/store/other".into()),
            Box::new(|p| p.plan.action = SystemCandidateAction::PreviewActivation),
            Box::new(|p| p.plan.effect_class = SystemEffectClass::PreviewHooks),
            Box::new(|p| p.plan.authority = SystemAuthorityClass::User),
            Box::new(|p| p.plan.program = "/bin/sh".into()),
            Box::new(|p| p.plan.args = vec!["switch".into()]),
            Box::new(|p| p.readiness_observed_at_unix_ms += 1),
            Box::new(|p| p.prepared_at_unix_ms += 1),
            Box::new(|p| p.authorization_expires_at_unix_ms += 1),
        ];

        for mutate in mutations {
            let mut changed = prepared(SystemCandidateAction::TestActivation);
            mutate(&mut changed);
            assert_ne!(
                baseline,
                TrustedActivationPermit::from_prepared(&changed).canonical_text()
            );
        }
    }

    fn temp_root() -> (PathBuf, u32) {
        let unique = format!(
            "blob-trusted-permit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let uid = fs::symlink_metadata(&root).unwrap().uid();
        (root, uid)
    }

    fn write_permit_for_test(
        store: &FileTrustedActivationPermitStore,
        request: &PreparedPrivilegedActivation,
    ) {
        let path = store.permit_path(&request.authorization);
        fs::write(
            &path,
            TrustedActivationPermit::from_prepared(request).canonical_text(),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn file_permit_is_exact_and_single_use() {
        let request = prepared(SystemCandidateAction::TestActivation);
        let (root, uid) = temp_root();
        let store = FileTrustedActivationPermitStore::new(&root, uid);
        write_permit_for_test(&store, &request);

        store.consume_matching(&request).expect("first exact use");
        assert!(matches!(
            store.consume_matching(&request),
            Err(TrustedActivationPermitError::MissingOrAlreadyConsumed(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mismatched_file_permit_is_not_consumed() {
        let request = prepared(SystemCandidateAction::TestActivation);
        let mut other = request.clone();
        other.plan.candidate = SystemCandidateId::from("candidate:other");
        let (root, uid) = temp_root();
        let store = FileTrustedActivationPermitStore::new(&root, uid);
        write_permit_for_test(&store, &request);

        assert_eq!(
            store.consume_matching(&other),
            Err(TrustedActivationPermitError::PermitMismatch)
        );
        store
            .consume_matching(&request)
            .expect("mismatch must not destroy the real permit");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsafe_permit_root_or_file_permissions_are_rejected() {
        let request = prepared(SystemCandidateAction::TestActivation);
        let (root, uid) = temp_root();
        let store = FileTrustedActivationPermitStore::new(&root, uid);
        write_permit_for_test(&store, &request);

        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            store.consume_matching(&request),
            Err(TrustedActivationPermitError::InvalidPermitRoot)
        );

        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let path = store.permit_path(&request.authorization);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            store.consume_matching(&request),
            Err(TrustedActivationPermitError::InvalidPermitFile)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wrong_owner_expectation_is_rejected() {
        let request = prepared(SystemCandidateAction::TestActivation);
        let (root, uid) = temp_root();
        let correct = FileTrustedActivationPermitStore::new(&root, uid);
        write_permit_for_test(&correct, &request);
        let wrong = FileTrustedActivationPermitStore::new(&root, uid.saturating_add(1));
        assert_eq!(
            wrong.consume_matching(&request),
            Err(TrustedActivationPermitError::InvalidPermitRoot)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_permit_file_is_rejected() {
        use std::os::unix::fs::symlink;

        let request = prepared(SystemCandidateAction::TestActivation);
        let (root, uid) = temp_root();
        let store = FileTrustedActivationPermitStore::new(&root, uid);
        let real = root.join("real-permit");
        fs::write(
            &real,
            TrustedActivationPermit::from_prepared(&request).canonical_text(),
        )
        .unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&real, store.permit_path(&request.authorization)).unwrap();

        assert_eq!(
            store.consume_matching(&request),
            Err(TrustedActivationPermitError::InvalidPermitFile)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
