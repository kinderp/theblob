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

    /// Canonical, injection-safe representation used by the root-owned file store.
    ///
    /// User-controlled strings are hex encoded so identifiers cannot smuggle
    /// additional key/value lines into a permit. The format is intentionally
    /// simple and versioned: a future incompatible format must use a new version.
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
    /// Authenticate and consume the exact privileged capability for `prepared`.
    ///
    /// Successful consumption is intentionally destructive. If execution later
    /// fails, a fresh OS-authenticated authorization must issue a new permit.
    fn consume_matching(
        &self,
        prepared: &PreparedPrivilegedActivation,
    ) -> Result<(), TrustedActivationPermitError>;
}

/// Non-production store useful for deterministic composition tests.
#[derive(Default)]
pub struct InMemoryTrustedActivationPermitStore {
    permits: Mutex<BTreeSet<String>>,
}

impl InMemoryTrustedActivationPermitStore {
    pub fn grant(&self, permit: TrustedActivationPermit) {
        self.permits
            .lock()
            .expect("in-memory permit store poisoned")
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

/// Read/consume side of the trusted privileged permit store.
///
/// This type deliberately has no public method that creates production permits.
/// Permit issuance belongs to a later, OS-authenticated privileged authorization
/// boundary. The execution helper can only verify and destroy a permit that is
/// already present in a protected directory.
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

    fn validate_file(&self, path: &Path) -> Result<(), TrustedActivationPermitError> {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                TrustedActivationPermitError::MissingOrAlreadyConsumed(
                    SystemAuthorizationId::from("missing"),
                )
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

        if let Err(error) = self.validate_file(&path) {
            return match error {
                TrustedActivationPermitError::MissingOrAlreadyConsumed(_) => Err(
                    TrustedActivationPermitError::MissingOrAlreadyConsumed(
                        prepared.authorization.clone(),
                    ),
                ),
                other => Err(other),
            };
        }

        let mut observed = String::new();
        File::open(&path)
            .and_then(|mut file| file.read_to_string(&mut observed))
            .map_err(|error| TrustedActivationPermitError::Io(error.to_string()))?;

        let expected = TrustedActivationPermit::from_prepared(prepared).canonical_text();
        if observed != expected {
            return Err(TrustedActivationPermitError::PermitMismatch);
        }

        // The protected directory is not writable by the unprivileged caller.
        // Removal is therefore the atomic winner for concurrent helper attempts:
        // exactly one process can successfully destroy the matching capability.
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

    use blob_nix_nixos_activation::ImmutableNixOsActivationPlan;

    use super::*;

    fn prepared() -> PreparedPrivilegedActivation {
        let closure = "/nix/store/candidate-nixos-system-blob-pilot";
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
                system_closure: closure.into(),
                action: SystemCandidateAction::TestActivation,
                effect_class: SystemEffectClass::TemporaryLiveActivation,
                authority: SystemAuthorityClass::HostAdministrator,
                program: format!("{closure}/bin/switch-to-configuration"),
                args: vec!["test".into()],
                expected_effects: vec![],
                rollback_semantics: "reboot restores baseline".into(),
            },
            readiness_evidence: vec![],
            authorization_evidence: vec![],
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

    fn write_permit_for_test(store: &FileTrustedActivationPermitStore, request: &PreparedPrivilegedActivation) {
        let path = store.permit_path(&request.authorization);
        fs::write(
            &path,
            TrustedActivationPermit::from_prepared(request).canonical_text(),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn canonical_permit_binds_every_execution_sensitive_field() {
        let baseline = TrustedActivationPermit::from_prepared(&prepared()).canonical_text();

        let mutations: Vec<Box<dyn Fn(&mut PreparedPrivilegedActivation)>> = vec![
            Box::new(|p| p.node = NodeId::from("node:other")),
            Box::new(|p| p.plan.operation_id = SystemOperationId::from("op:other")),
            Box::new(|p| p.plan.candidate = SystemCandidateId::from("candidate:other")),
            Box::new(|p| p.plan.system_spec = SystemSpecId::from("system:other")),
            Box::new(|p| p.plan.materialization_operation = SystemOperationId::from("op:other-materialize")),
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
            let mut changed = prepared();
            mutate(&mut changed);
            assert_ne!(
                baseline,
                TrustedActivationPermit::from_prepared(&changed).canonical_text()
            );
        }
    }

    #[test]
    fn in_memory_permit_is_exact_and_single_use() {
        let request = prepared();
        let store = InMemoryTrustedActivationPermitStore::default();
        store.grant_for(&request);
        store.consume_matching(&request).expect("first exact use");
        assert!(matches!(
            store.consume_matching(&request),
            Err(TrustedActivationPermitError::MissingOrAlreadyConsumed(_))
        ));
    }

    #[test]
    fn forged_request_does_not_consume_a_real_permit() {
        let request = prepared();
        let mut forged = request.clone();
        forged.plan.system_closure = "/nix/store/forged".into();
        forged.plan.program = "/nix/store/forged/bin/switch-to-configuration".into();

        let store = InMemoryTrustedActivationPermitStore::default();
        store.grant_for(&request);
        assert!(matches!(
            store.consume_matching(&forged),
            Err(TrustedActivationPermitError::MissingOrAlreadyConsumed(_))
        ));
        store
            .consume_matching(&request)
            .expect("real permit remains available");
    }

    #[test]
    fn file_store_consumes_exact_root_owned_equivalent_permit_once() {
        let request = prepared();
        let (root, uid) = temp_root();
        let store = FileTrustedActivationPermitStore::new(&root, uid);
        write_permit_for_test(&store, &request);

        store.consume_matching(&request).expect("first use");
        assert!(matches!(
            store.consume_matching(&request),
            Err(TrustedActivationPermitError::MissingOrAlreadyConsumed(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mismatched_file_permit_is_not_consumed() {
        let request = prepared();
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
            .expect("mismatch must not destroy real permit");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn permissive_or_wrong_owner_root_is_rejected() {
        let request = prepared();
        let (root, uid) = temp_root();
        let store = FileTrustedActivationPermitStore::new(&root, uid);
        write_permit_for_test(&store, &request);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            store.consume_matching(&request),
            Err(TrustedActivationPermitError::InvalidPermitRoot)
        );

        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let wrong_owner = FileTrustedActivationPermitStore::new(&root, uid.saturating_add(1));
        assert_eq!(
            wrong_owner.consume_matching(&request),
            Err(TrustedActivationPermitError::InvalidPermitRoot)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn permissive_permit_file_is_rejected() {
        let request = prepared();
        let (root, uid) = temp_root();
        let store = FileTrustedActivationPermitStore::new(&root, uid);
        write_permit_for_test(&store, &request);
        let path = store.permit_path(&request.authorization);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            store.consume_matching(&request),
            Err(TrustedActivationPermitError::InvalidPermitFile)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
