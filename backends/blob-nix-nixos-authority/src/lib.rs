#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use blob_core::{NodeId, SystemAuthorizationId, SystemCandidateAction};
use blob_nix_nixos_privileged_helper::{
    PrivilegedActivationError, PrivilegedActivationRuntimePolicy, PrivilegedNixOsActivationHelper,
};
use blob_nix_nixos_root_boundary::{
    TrustedActivationPermit, DEFAULT_TRUSTED_PERMIT_ROOT,
};
use blob_system_activation_gate::PreparedPrivilegedActivation;

pub const PREVIEW_POLKIT_ACTION_ID: &str = "org.theblob.nixos.preview-activation";
pub const TEST_POLKIT_ACTION_ID: &str = "org.theblob.nixos.test-activation";
pub const DEFAULT_MAX_POLKIT_GRANT_AGE_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolkitActivationAction {
    PreviewActivation,
    TestActivation,
}

impl PolkitActivationAction {
    pub fn action_id(self) -> &'static str {
        match self {
            Self::PreviewActivation => PREVIEW_POLKIT_ACTION_ID,
            Self::TestActivation => TEST_POLKIT_ACTION_ID,
        }
    }

    pub fn from_prepared(
        prepared: &PreparedPrivilegedActivation,
    ) -> Result<Self, PolkitAuthorizationError> {
        match prepared.plan.action {
            SystemCandidateAction::PreviewActivation => Ok(Self::PreviewActivation),
            SystemCandidateAction::TestActivation => Ok(Self::TestActivation),
            ref other => Err(PolkitAuthorizationError::UnsupportedAction(other.clone())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolkitAuthorizationRequest {
    system_bus_name: String,
    action: PolkitActivationAction,
    approved_permit: TrustedActivationPermit,
}

impl PolkitAuthorizationRequest {
    pub fn for_prepared(
        system_bus_name: impl Into<String>,
        prepared: &PreparedPrivilegedActivation,
    ) -> Result<Self, PolkitAuthorizationError> {
        let system_bus_name = system_bus_name.into();
        if !valid_unique_system_bus_name(&system_bus_name) {
            return Err(PolkitAuthorizationError::InvalidSystemBusName);
        }
        Ok(Self {
            system_bus_name,
            action: PolkitActivationAction::from_prepared(prepared)?,
            approved_permit: TrustedActivationPermit::from_prepared(prepared),
        })
    }

    pub fn system_bus_name(&self) -> &str {
        &self.system_bus_name
    }

    pub fn action(&self) -> PolkitActivationAction {
        self.action
    }

    pub fn approved_permit(&self) -> &TrustedActivationPermit {
        &self.approved_permit
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolkitCheckPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl PolkitCheckPlan {
    pub fn new(
        pkcheck_program: impl Into<PathBuf>,
        request: &PolkitAuthorizationRequest,
    ) -> Result<Self, PolkitAuthorizationError> {
        let program = pkcheck_program.into();
        if !program.is_absolute() || program.file_name().and_then(|name| name.to_str()) != Some("pkcheck") {
            return Err(PolkitAuthorizationError::InvalidPkcheckProgram);
        }

        let permit = request.approved_permit();
        Ok(Self {
            program,
            args: vec![
                "--action-id".into(),
                request.action().action_id().into(),
                "--system-bus-name".into(),
                request.system_bus_name().into(),
                "--allow-user-interaction".into(),
                "--detail".into(),
                "theblob_node".into(),
                permit.node.to_string(),
                "--detail".into(),
                "theblob_candidate".into(),
                permit.candidate.to_string(),
                "--detail".into(),
                "theblob_system_spec".into(),
                permit.system_spec.to_string(),
                "--detail".into(),
                "theblob_action".into(),
                request.action().action_id().into(),
                "--detail".into(),
                "theblob_system_closure".into(),
                permit.system_closure.clone(),
            ],
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PkcheckCommandOutcome {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub trait PkcheckCommandRunner {
    fn run(&self, plan: &PolkitCheckPlan) -> Result<PkcheckCommandOutcome, String>;
}

pub struct StdPkcheckCommandRunner;

impl PkcheckCommandRunner for StdPkcheckCommandRunner {
    fn run(&self, plan: &PolkitCheckPlan) -> Result<PkcheckCommandOutcome, String> {
        let output = Command::new(&plan.program)
            .args(&plan.args)
            .stdin(Stdio::null())
            .env_clear()
            .env("LANG", "C")
            .output()
            .map_err(|error| error.to_string())?;
        Ok(PkcheckCommandOutcome {
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolkitAuthorizationError {
    InvalidSystemBusName,
    InvalidPkcheckProgram,
    UnsupportedAction(SystemCandidateAction),
    Denied(String),
    NoAuthenticationAgent(String),
    Dismissed(String),
    CheckFailed { exit_code: Option<i32>, stderr: String },
    Spawn(String),
}

/// A successful OS authorization result. Fields are intentionally private and
/// there is no public constructor; callers obtain this only from a checker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OsAuthorizationGrant {
    system_bus_name: String,
    action: PolkitActivationAction,
    approved_permit_canonical: String,
    checked_at_unix_ms: u64,
}

impl OsAuthorizationGrant {
    pub fn system_bus_name(&self) -> &str {
        &self.system_bus_name
    }

    pub fn action(&self) -> PolkitActivationAction {
        self.action
    }

    pub fn checked_at_unix_ms(&self) -> u64 {
        self.checked_at_unix_ms
    }
}

pub struct PkcheckAuthorizationChecker<R> {
    program: PathBuf,
    runner: R,
}

impl<R: PkcheckCommandRunner> PkcheckAuthorizationChecker<R> {
    pub fn new(
        pkcheck_program: impl Into<PathBuf>,
        runner: R,
    ) -> Result<Self, PolkitAuthorizationError> {
        let program = pkcheck_program.into();
        if !program.is_absolute() || program.file_name().and_then(|name| name.to_str()) != Some("pkcheck") {
            return Err(PolkitAuthorizationError::InvalidPkcheckProgram);
        }
        Ok(Self { program, runner })
    }

    /// Check a user-initiated request. `--allow-user-interaction` is always
    /// present; a future daemon must run this blocking operation on a dedicated
    /// worker rather than its dispatch thread.
    pub fn check_user_initiated(
        &self,
        request: &PolkitAuthorizationRequest,
        now_unix_ms: u64,
    ) -> Result<OsAuthorizationGrant, PolkitAuthorizationError> {
        let plan = PolkitCheckPlan::new(&self.program, request)?;
        let outcome = self
            .runner
            .run(&plan)
            .map_err(PolkitAuthorizationError::Spawn)?;
        match outcome.exit_code {
            Some(0) => Ok(OsAuthorizationGrant {
                system_bus_name: request.system_bus_name.clone(),
                action: request.action,
                approved_permit_canonical: request.approved_permit.canonical_text(),
                checked_at_unix_ms: now_unix_ms,
            }),
            Some(1) => Err(PolkitAuthorizationError::Denied(outcome.stderr)),
            Some(2) => Err(PolkitAuthorizationError::NoAuthenticationAgent(
                outcome.stderr,
            )),
            Some(3) => Err(PolkitAuthorizationError::Dismissed(outcome.stderr)),
            exit_code => Err(PolkitAuthorizationError::CheckFailed {
                exit_code,
                stderr: outcome.stderr,
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssuedActivationPermit {
    pub authorization: SystemAuthorizationId,
    pub authorized_system_bus_name: String,
    pub action_id: String,
    pub permit_path: PathBuf,
    pub issued_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PermitIssueError {
    PreparedRejected(PrivilegedActivationError),
    UnsupportedAction(SystemCandidateAction),
    GrantTimestampInvalid,
    GrantStale,
    GrantActionMismatch,
    GrantExactBindingMismatch,
    InvalidPermitRoot,
    AlreadyIssued(SystemAuthorizationId),
    InvalidCreatedPermit,
    Io(String),
}

pub struct RootOwnedActivationPermitIssuer {
    root: PathBuf,
    expected_owner_uid: u32,
    max_grant_age_ms: u64,
    validator: PrivilegedNixOsActivationHelper,
}

impl RootOwnedActivationPermitIssuer {
    pub fn production_default(local_node: impl Into<NodeId>) -> Self {
        Self::new(local_node, DEFAULT_TRUSTED_PERMIT_ROOT, 0)
    }

    pub fn new(
        local_node: impl Into<NodeId>,
        root: impl Into<PathBuf>,
        expected_owner_uid: u32,
    ) -> Self {
        Self {
            root: root.into(),
            expected_owner_uid,
            max_grant_age_ms: DEFAULT_MAX_POLKIT_GRANT_AGE_MS,
            validator: PrivilegedNixOsActivationHelper::new(local_node),
        }
    }

    pub fn with_policy(
        local_node: impl Into<NodeId>,
        root: impl Into<PathBuf>,
        expected_owner_uid: u32,
        max_grant_age_ms: u64,
        runtime_policy: PrivilegedActivationRuntimePolicy,
    ) -> Self {
        Self {
            root: root.into(),
            expected_owner_uid,
            max_grant_age_ms,
            validator: PrivilegedNixOsActivationHelper::with_policy(local_node, runtime_policy),
        }
    }

    pub fn issue(
        &self,
        prepared: &PreparedPrivilegedActivation,
        grant: &OsAuthorizationGrant,
        now_unix_ms: u64,
    ) -> Result<IssuedActivationPermit, PermitIssueError> {
        self.validator
            .validate_prepared(prepared, now_unix_ms)
            .map_err(PermitIssueError::PreparedRejected)?;

        if grant.checked_at_unix_ms < prepared.prepared_at_unix_ms
            || grant.checked_at_unix_ms > now_unix_ms
        {
            return Err(PermitIssueError::GrantTimestampInvalid);
        }
        if now_unix_ms.saturating_sub(grant.checked_at_unix_ms) > self.max_grant_age_ms {
            return Err(PermitIssueError::GrantStale);
        }

        let expected_action = PolkitActivationAction::from_prepared(prepared)
            .map_err(|error| match error {
                PolkitAuthorizationError::UnsupportedAction(action) => {
                    PermitIssueError::UnsupportedAction(action)
                }
                _ => unreachable!(),
            })?;
        if grant.action != expected_action {
            return Err(PermitIssueError::GrantActionMismatch);
        }

        let permit = TrustedActivationPermit::from_prepared(prepared);
        let canonical = permit.canonical_text();
        if grant.approved_permit_canonical != canonical {
            return Err(PermitIssueError::GrantExactBindingMismatch);
        }

        self.validate_root()?;
        let path = self.permit_path(&prepared.authorization);
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(PermitIssueError::AlreadyIssued(
                    prepared.authorization.clone(),
                ));
            }
            Err(error) => return Err(PermitIssueError::Io(error.to_string())),
        };

        if let Err(error) = file
            .write_all(canonical.as_bytes())
            .and_then(|_| file.sync_all())
        {
            let _ = fs::remove_file(&path);
            return Err(PermitIssueError::Io(error.to_string()));
        }

        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| PermitIssueError::Io(error.to_string()))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != self.expected_owner_uid
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            let _ = fs::remove_file(&path);
            return Err(PermitIssueError::InvalidCreatedPermit);
        }

        File::open(&self.root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| PermitIssueError::Io(error.to_string()))?;

        Ok(IssuedActivationPermit {
            authorization: prepared.authorization.clone(),
            authorized_system_bus_name: grant.system_bus_name.clone(),
            action_id: expected_action.action_id().into(),
            permit_path: path,
            issued_at_unix_ms: now_unix_ms,
        })
    }

    fn validate_root(&self) -> Result<(), PermitIssueError> {
        let metadata = fs::symlink_metadata(&self.root)
            .map_err(|error| PermitIssueError::Io(error.to_string()))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.uid() != self.expected_owner_uid
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(PermitIssueError::InvalidPermitRoot);
        }
        Ok(())
    }

    fn permit_path(&self, authorization: &SystemAuthorizationId) -> PathBuf {
        self.root.join(format!(
            "authorization-{}.permit",
            hex_text(authorization.as_str())
        ))
    }
}

fn valid_unique_system_bus_name(name: &str) -> bool {
    if name.len() < 4 || name.len() > 255 || !name.starts_with(':') {
        return false;
    }
    let parts = name[1..].split('.').collect::<Vec<_>>();
    parts.len() >= 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        })
}

fn hex_text(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    use blob_core::{
        SystemAuthorityClass, SystemCandidateId, SystemEffectClass, SystemOperationId,
        SystemSpecId,
    };
    use blob_nix_nixos_activation::ImmutableNixOsActivationPlan;
    use blob_nix_nixos_root_boundary::{
        FileTrustedActivationPermitStore, TrustedActivationPermitStore,
    };

    use super::*;

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

    #[derive(Clone)]
    struct FakeRunner {
        outcome: PkcheckCommandOutcome,
    }

    impl PkcheckCommandRunner for FakeRunner {
        fn run(&self, _plan: &PolkitCheckPlan) -> Result<PkcheckCommandOutcome, String> {
            Ok(self.outcome.clone())
        }
    }

    fn checker(exit_code: Option<i32>) -> PkcheckAuthorizationChecker<FakeRunner> {
        PkcheckAuthorizationChecker::new(
            "/nix/store/polkit/bin/pkcheck",
            FakeRunner {
                outcome: PkcheckCommandOutcome {
                    exit_code,
                    stdout: String::new(),
                    stderr: "diagnostic".into(),
                },
            },
        )
        .unwrap()
    }

    #[test]
    fn request_accepts_only_unique_system_bus_names_and_live_actions() {
        let request = PolkitAuthorizationRequest::for_prepared(
            ":1.42",
            &prepared(SystemCandidateAction::TestActivation),
        )
        .expect("valid request");
        assert_eq!(request.action().action_id(), TEST_POLKIT_ACTION_ID);

        assert_eq!(
            PolkitAuthorizationRequest::for_prepared(
                "org.example.Client",
                &prepared(SystemCandidateAction::TestActivation)
            ),
            Err(PolkitAuthorizationError::InvalidSystemBusName)
        );
    }

    #[test]
    fn pkcheck_plan_uses_system_bus_identity_and_never_pid_or_shell() {
        let request = PolkitAuthorizationRequest::for_prepared(
            ":1.42",
            &prepared(SystemCandidateAction::TestActivation),
        )
        .unwrap();
        let plan = PolkitCheckPlan::new("/nix/store/polkit/bin/pkcheck", &request).unwrap();

        assert_eq!(plan.program, PathBuf::from("/nix/store/polkit/bin/pkcheck"));
        assert!(plan.args.windows(2).any(|pair| pair == ["--system-bus-name", ":1.42"]));
        assert!(plan.args.contains(&"--allow-user-interaction".into()));
        assert!(!plan.args.contains(&"--process".into()));
        assert!(!plan.args.iter().any(|arg| arg == "sh" || arg == "bash"));
    }

    #[test]
    fn successful_pkcheck_grant_is_exactly_bound_to_requested_permit() {
        let prepared = prepared(SystemCandidateAction::TestActivation);
        let request = PolkitAuthorizationRequest::for_prepared(":1.42", &prepared).unwrap();
        let grant = checker(Some(0))
            .check_user_initiated(&request, 2_100)
            .expect("authorized");

        assert_eq!(grant.system_bus_name(), ":1.42");
        assert_eq!(grant.action().action_id(), TEST_POLKIT_ACTION_ID);
        assert_eq!(grant.checked_at_unix_ms(), 2_100);
        assert_eq!(
            grant.approved_permit_canonical,
            TrustedActivationPermit::from_prepared(&prepared).canonical_text()
        );
    }

    #[test]
    fn pkcheck_exit_codes_fail_closed() {
        let request = PolkitAuthorizationRequest::for_prepared(
            ":1.42",
            &prepared(SystemCandidateAction::PreviewActivation),
        )
        .unwrap();
        assert!(matches!(
            checker(Some(1)).check_user_initiated(&request, 2_100),
            Err(PolkitAuthorizationError::Denied(_))
        ));
        assert!(matches!(
            checker(Some(2)).check_user_initiated(&request, 2_100),
            Err(PolkitAuthorizationError::NoAuthenticationAgent(_))
        ));
        assert!(matches!(
            checker(Some(3)).check_user_initiated(&request, 2_100),
            Err(PolkitAuthorizationError::Dismissed(_))
        ));
        assert!(matches!(
            checker(Some(127)).check_user_initiated(&request, 2_100),
            Err(PolkitAuthorizationError::CheckFailed { .. })
        ));
    }

    fn temp_root() -> (PathBuf, u32) {
        let unique = format!(
            "blob-authority-permits-{}-{}",
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

    fn grant_for(prepared: &PreparedPrivilegedActivation, checked_at: u64) -> OsAuthorizationGrant {
        let request = PolkitAuthorizationRequest::for_prepared(":1.42", prepared).unwrap();
        checker(Some(0))
            .check_user_initiated(&request, checked_at)
            .unwrap()
    }

    #[test]
    fn issuer_output_is_consumable_by_root_boundary_store() {
        let prepared = prepared(SystemCandidateAction::TestActivation);
        let grant = grant_for(&prepared, 2_100);
        let (root, uid) = temp_root();
        let issuer = RootOwnedActivationPermitIssuer::new("node:lab", &root, uid);

        let issued = issuer.issue(&prepared, &grant, 2_200).expect("issued");
        assert_eq!(issued.authorized_system_bus_name, ":1.42");
        assert_eq!(issued.action_id, TEST_POLKIT_ACTION_ID);

        let store = FileTrustedActivationPermitStore::new(&root, uid);
        store
            .consume_matching(&prepared)
            .expect("root boundary must accept issuer output");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn grant_cannot_be_rebound_to_another_candidate_after_authentication() {
        let original = prepared(SystemCandidateAction::TestActivation);
        let grant = grant_for(&original, 2_100);
        let mut forged = original.clone();
        forged.plan.candidate = SystemCandidateId::from("candidate:forged");
        let (root, uid) = temp_root();
        let issuer = RootOwnedActivationPermitIssuer::new("node:lab", &root, uid);

        assert_eq!(
            issuer.issue(&forged, &grant, 2_200),
            Err(PermitIssueError::GrantExactBindingMismatch)
        );
        assert!(fs::read_dir(&root).unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_or_prepared_after_grant_is_rejected() {
        let prepared = prepared(SystemCandidateAction::PreviewActivation);
        let grant = grant_for(&prepared, 2_100);
        let (root, uid) = temp_root();
        let issuer = RootOwnedActivationPermitIssuer::with_policy(
            "node:lab",
            &root,
            uid,
            10,
            PrivilegedActivationRuntimePolicy {
                max_prepared_age_ms: u64::MAX,
                max_readiness_age_ms: u64::MAX,
            },
        );
        assert_eq!(
            issuer.issue(&prepared, &grant, 2_111),
            Err(PermitIssueError::GrantStale)
        );

        let request = PolkitAuthorizationRequest::for_prepared(":1.42", &prepared).unwrap();
        let before_prepared = checker(Some(0))
            .check_user_initiated(&request, 1_999)
            .unwrap();
        assert_eq!(
            issuer.issue(&prepared, &before_prepared, 2_005),
            Err(PermitIssueError::GrantTimestampInvalid)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn issuer_refuses_unsafe_root_and_never_overwrites_existing_permit() {
        let prepared = prepared(SystemCandidateAction::TestActivation);
        let grant = grant_for(&prepared, 2_100);
        let (root, uid) = temp_root();
        let issuer = RootOwnedActivationPermitIssuer::new("node:lab", &root, uid);

        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            issuer.issue(&prepared, &grant, 2_200),
            Err(PermitIssueError::InvalidPermitRoot)
        );

        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        issuer.issue(&prepared, &grant, 2_200).expect("first issue");
        assert!(matches!(
            issuer.issue(&prepared, &grant, 2_201),
            Err(PermitIssueError::AlreadyIssued(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
