#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use blob_core::{
    NodeId, SystemAuthorityClass, SystemAuthorizationId, SystemCandidateAction, SystemCandidateId,
    SystemEffectClass, SystemOperationId, SystemSpecId,
};
use blob_nix_nixos_activation::ImmutableNixOsActivationPlan;
use blob_system_activation_gate::PreparedPrivilegedActivation;

pub const DEFAULT_PREPARED_REQUEST_ROOT: &str = "/var/lib/theblob/prepared-activations";
pub const MAX_PREPARED_REQUEST_BYTES: u64 = 64 * 1024;

const READY: &str = "ready";
const INFLIGHT: &str = "inflight";
const COMPLETED: &str = "completed";
const FAILED: &str = "failed";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreparedActivationRequestStoreError {
    MissingReady(SystemAuthorizationId),
    AlreadyClaimed(SystemAuthorizationId),
    TerminalState(SystemAuthorizationId),
    RequestMismatch,
    RequestChangedAfterAuthorization,
    InvalidLayout,
    InvalidRequestFile,
    RequestTooLarge,
    NonCanonicalRequest,
    Malformed(String),
    Io(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimedPreparedActivationRequest {
    pub prepared: PreparedPrivilegedActivation,
    root: PathBuf,
    expected_owner_uid: u32,
}

pub struct FilePreparedActivationRequestStore {
    root: PathBuf,
    expected_owner_uid: u32,
}

impl FilePreparedActivationRequestStore {
    pub fn production_default() -> Self {
        Self::new(DEFAULT_PREPARED_REQUEST_ROOT, 0)
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

    /// Read only. This phase is for semantic validation and OS authorization.
    /// Privileged permit issuance is forbidden until `claim_exact` succeeds.
    pub fn load_ready(
        &self,
        authorization: &SystemAuthorizationId,
    ) -> Result<PreparedPrivilegedActivation, PreparedActivationRequestStoreError> {
        self.validate_layout()?;
        self.reject_non_ready_state(authorization)?;
        self.read_request_path(&self.request_path(READY, authorization), authorization)
    }

    /// Durably claim exactly the request that polkit already authorized.
    ///
    /// O_CREAT|O_EXCL creates the inflight receipt before any move. A crash can
    /// therefore strand liveness, but cannot make a claimed request ready again.
    /// The request is re-read after that durable receipt to close the
    /// load->authorize->claim TOCTOU window before the atomic rename.
    pub fn claim_exact(
        &self,
        expected: &PreparedPrivilegedActivation,
    ) -> Result<ClaimedPreparedActivationRequest, PreparedActivationRequestStoreError> {
        self.validate_layout()?;
        self.reject_non_ready_state(&expected.authorization)?;

        let ready_path = self.request_path(READY, &expected.authorization);
        let before = self.read_request_path(&ready_path, &expected.authorization)?;
        if before != *expected {
            return Err(PreparedActivationRequestStoreError::RequestMismatch);
        }

        let claim_path = self.claim_path(INFLIGHT, &expected.authorization);
        let mut claim = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&claim_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(PreparedActivationRequestStoreError::AlreadyClaimed(
                    expected.authorization.clone(),
                ));
            }
            Err(error) => return Err(PreparedActivationRequestStoreError::Io(error.to_string())),
        };
        writeln!(claim, "theblob-prepared-activation-claim-v1")
            .and_then(|_| {
                writeln!(
                    claim,
                    "authorization={}",
                    hex_text(expected.authorization.as_str())
                )
            })
            .and_then(|_| {
                writeln!(
                    claim,
                    "prepared-at-unix-ms={}",
                    expected.prepared_at_unix_ms
                )
            })
            .and_then(|_| claim.sync_all())
            .map_err(|error| PreparedActivationRequestStoreError::Io(error.to_string()))?;
        sync_dir(&self.state_dir(INFLIGHT))?;

        let after = self
            .read_request_path(&ready_path, &expected.authorization)
            .map_err(|error| match error {
                PreparedActivationRequestStoreError::MissingReady(_) => {
                    PreparedActivationRequestStoreError::RequestChangedAfterAuthorization
                }
                other => other,
            })?;
        if after != *expected {
            return Err(PreparedActivationRequestStoreError::RequestChangedAfterAuthorization);
        }

        let inflight_path = self.request_path(INFLIGHT, &expected.authorization);
        if path_exists(&inflight_path)? {
            return Err(PreparedActivationRequestStoreError::AlreadyClaimed(
                expected.authorization.clone(),
            ));
        }
        fs::rename(&ready_path, &inflight_path)
            .map_err(|error| PreparedActivationRequestStoreError::Io(error.to_string()))?;
        sync_dir(&self.state_dir(READY))?;
        sync_dir(&self.state_dir(INFLIGHT))?;

        Ok(ClaimedPreparedActivationRequest {
            prepared: expected.clone(),
            root: self.root.clone(),
            expected_owner_uid: self.expected_owner_uid,
        })
    }

    pub fn mark_completed(
        &self,
        claimed: &ClaimedPreparedActivationRequest,
    ) -> Result<(), PreparedActivationRequestStoreError> {
        self.mark_terminal(claimed, COMPLETED)
    }

    pub fn mark_failed(
        &self,
        claimed: &ClaimedPreparedActivationRequest,
    ) -> Result<(), PreparedActivationRequestStoreError> {
        self.mark_terminal(claimed, FAILED)
    }

    fn mark_terminal(
        &self,
        claimed: &ClaimedPreparedActivationRequest,
        terminal: &str,
    ) -> Result<(), PreparedActivationRequestStoreError> {
        if claimed.root != self.root || claimed.expected_owner_uid != self.expected_owner_uid {
            return Err(PreparedActivationRequestStoreError::RequestMismatch);
        }
        self.validate_layout()?;

        let authorization = &claimed.prepared.authorization;
        let inflight_request = self.request_path(INFLIGHT, authorization);
        let observed = self.read_request_path(&inflight_request, authorization)?;
        if observed != claimed.prepared {
            return Err(PreparedActivationRequestStoreError::RequestMismatch);
        }

        let terminal_request = self.request_path(terminal, authorization);
        let terminal_claim = self.claim_path(terminal, authorization);
        if path_exists(&terminal_request)? || path_exists(&terminal_claim)? {
            return Err(PreparedActivationRequestStoreError::TerminalState(
                authorization.clone(),
            ));
        }

        fs::rename(&inflight_request, &terminal_request)
            .map_err(|error| PreparedActivationRequestStoreError::Io(error.to_string()))?;
        sync_dir(&self.state_dir(INFLIGHT))?;
        sync_dir(&self.state_dir(terminal))?;

        let inflight_claim = self.claim_path(INFLIGHT, authorization);
        fs::rename(&inflight_claim, &terminal_claim)
            .map_err(|error| PreparedActivationRequestStoreError::Io(error.to_string()))?;
        sync_dir(&self.state_dir(INFLIGHT))?;
        sync_dir(&self.state_dir(terminal))?;
        Ok(())
    }

    fn reject_non_ready_state(
        &self,
        authorization: &SystemAuthorizationId,
    ) -> Result<(), PreparedActivationRequestStoreError> {
        if path_exists(&self.request_path(COMPLETED, authorization))?
            || path_exists(&self.claim_path(COMPLETED, authorization))?
            || path_exists(&self.request_path(FAILED, authorization))?
            || path_exists(&self.claim_path(FAILED, authorization))?
        {
            return Err(PreparedActivationRequestStoreError::TerminalState(
                authorization.clone(),
            ));
        }
        if path_exists(&self.request_path(INFLIGHT, authorization))?
            || path_exists(&self.claim_path(INFLIGHT, authorization))?
        {
            return Err(PreparedActivationRequestStoreError::AlreadyClaimed(
                authorization.clone(),
            ));
        }
        Ok(())
    }

    fn read_request_path(
        &self,
        path: &Path,
        authorization: &SystemAuthorizationId,
    ) -> Result<PreparedPrivilegedActivation, PreparedActivationRequestStoreError> {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                PreparedActivationRequestStoreError::MissingReady(authorization.clone())
            } else {
                PreparedActivationRequestStoreError::Io(error.to_string())
            }
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != self.expected_owner_uid
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(PreparedActivationRequestStoreError::InvalidRequestFile);
        }
        if metadata.len() > MAX_PREPARED_REQUEST_BYTES {
            return Err(PreparedActivationRequestStoreError::RequestTooLarge);
        }

        let mut text = String::new();
        File::open(path)
            .and_then(|mut file| file.read_to_string(&mut text))
            .map_err(|error| PreparedActivationRequestStoreError::Io(error.to_string()))?;
        let prepared = parse_canonical_text(&text)?;
        if prepared.authorization != *authorization {
            return Err(PreparedActivationRequestStoreError::RequestMismatch);
        }
        if canonical_text(&prepared) != text {
            return Err(PreparedActivationRequestStoreError::NonCanonicalRequest);
        }
        Ok(prepared)
    }

    fn validate_layout(&self) -> Result<(), PreparedActivationRequestStoreError> {
        validate_directory(&self.root, self.expected_owner_uid)?;
        for state in [READY, INFLIGHT, COMPLETED, FAILED] {
            validate_directory(&self.state_dir(state), self.expected_owner_uid)?;
        }
        Ok(())
    }

    fn state_dir(&self, state: &str) -> PathBuf {
        self.root.join(state)
    }

    fn request_path(&self, state: &str, authorization: &SystemAuthorizationId) -> PathBuf {
        self.state_dir(state).join(format!(
            "authorization-{}.request",
            hex_text(authorization.as_str())
        ))
    }

    fn claim_path(&self, state: &str, authorization: &SystemAuthorizationId) -> PathBuf {
        self.state_dir(state).join(format!(
            "authorization-{}.claim",
            hex_text(authorization.as_str())
        ))
    }
}

/// Versioned, field-ordered, injection-safe representation. Text values are
/// lowercase hex; unknown, duplicated, reordered or trailing fields are rejected.
pub fn canonical_text(prepared: &PreparedPrivilegedActivation) -> String {
    let mut lines = vec![
        "theblob-prepared-activation-v1".to_owned(),
        format!("authorization={}", hex_text(prepared.authorization.as_str())),
        format!("node={}", hex_text(prepared.node.as_str())),
        format!(
            "readiness-observed-at-unix-ms={}",
            prepared.readiness_observed_at_unix_ms
        ),
        format!(
            "authorization-expires-at-unix-ms={}",
            prepared.authorization_expires_at_unix_ms
        ),
        format!("prepared-at-unix-ms={}", prepared.prepared_at_unix_ms),
        format!("operation={}", hex_text(prepared.plan.operation_id.as_str())),
        format!("candidate={}", hex_text(prepared.plan.candidate.as_str())),
        format!("system-spec={}", hex_text(prepared.plan.system_spec.as_str())),
        format!(
            "materialization-operation={}",
            hex_text(prepared.plan.materialization_operation.as_str())
        ),
        format!("action={}", action_token(&prepared.plan.action)),
        format!("effect-class={}", effect_token(&prepared.plan.effect_class)),
        format!("authority={}", authority_token(&prepared.plan.authority)),
        format!("system-closure={}", hex_text(&prepared.plan.system_closure)),
        format!("program={}", hex_text(&prepared.plan.program)),
        format!("args-count={}", prepared.plan.args.len()),
    ];
    for (index, value) in prepared.plan.args.iter().enumerate() {
        lines.push(format!("arg-{index}={}", hex_text(value)));
    }
    lines.push(format!(
        "expected-effects-count={}",
        prepared.plan.expected_effects.len()
    ));
    for (index, value) in prepared.plan.expected_effects.iter().enumerate() {
        lines.push(format!("expected-effect-{index}={}", hex_text(value)));
    }
    lines.push(format!(
        "rollback-semantics={}",
        hex_text(&prepared.plan.rollback_semantics)
    ));
    lines.push(format!(
        "readiness-evidence-count={}",
        prepared.readiness_evidence.len()
    ));
    for (index, value) in prepared.readiness_evidence.iter().enumerate() {
        lines.push(format!("readiness-evidence-{index}={}", hex_text(value)));
    }
    lines.push(format!(
        "authorization-evidence-count={}",
        prepared.authorization_evidence.len()
    ));
    for (index, value) in prepared.authorization_evidence.iter().enumerate() {
        lines.push(format!(
            "authorization-evidence-{index}={}",
            hex_text(value)
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

pub fn parse_canonical_text(
    text: &str,
) -> Result<PreparedPrivilegedActivation, PreparedActivationRequestStoreError> {
    if text.len() as u64 > MAX_PREPARED_REQUEST_BYTES {
        return Err(PreparedActivationRequestStoreError::RequestTooLarge);
    }
    let mut cursor = LineCursor::new(text);
    cursor.expect_literal("theblob-prepared-activation-v1")?;

    let authorization = SystemAuthorizationId::from(cursor.hex_field("authorization")?);
    let node = NodeId::from(cursor.hex_field("node")?);
    let readiness_observed_at_unix_ms = cursor.u64_field("readiness-observed-at-unix-ms")?;
    let authorization_expires_at_unix_ms =
        cursor.u64_field("authorization-expires-at-unix-ms")?;
    let prepared_at_unix_ms = cursor.u64_field("prepared-at-unix-ms")?;
    let operation_id = SystemOperationId::from(cursor.hex_field("operation")?);
    let candidate = SystemCandidateId::from(cursor.hex_field("candidate")?);
    let system_spec = SystemSpecId::from(cursor.hex_field("system-spec")?);
    let materialization_operation =
        SystemOperationId::from(cursor.hex_field("materialization-operation")?);
    let action = parse_action(cursor.field("action")?)?;
    let effect_class = parse_effect(cursor.field("effect-class")?)?;
    let authority = parse_authority(cursor.field("authority")?)?;
    let system_closure = cursor.hex_field("system-closure")?;
    let program = cursor.hex_field("program")?;

    let args_count = cursor.count_field("args-count", 16)?;
    let mut args = Vec::with_capacity(args_count);
    for index in 0..args_count {
        args.push(cursor.hex_field(&format!("arg-{index}"))?);
    }

    let expected_count = cursor.count_field("expected-effects-count", 256)?;
    let mut expected_effects = Vec::with_capacity(expected_count);
    for index in 0..expected_count {
        expected_effects.push(cursor.hex_field(&format!("expected-effect-{index}"))?);
    }
    let rollback_semantics = cursor.hex_field("rollback-semantics")?;

    let readiness_count = cursor.count_field("readiness-evidence-count", 256)?;
    let mut readiness_evidence = Vec::with_capacity(readiness_count);
    for index in 0..readiness_count {
        readiness_evidence.push(cursor.hex_field(&format!("readiness-evidence-{index}"))?);
    }

    let authorization_count = cursor.count_field("authorization-evidence-count", 256)?;
    let mut authorization_evidence = Vec::with_capacity(authorization_count);
    for index in 0..authorization_count {
        authorization_evidence.push(cursor.hex_field(&format!("authorization-evidence-{index}"))?);
    }
    cursor.finish()?;

    Ok(PreparedPrivilegedActivation {
        node,
        readiness_observed_at_unix_ms,
        authorization,
        authorization_expires_at_unix_ms,
        prepared_at_unix_ms,
        plan: ImmutableNixOsActivationPlan {
            operation_id,
            candidate,
            system_spec,
            materialization_operation,
            system_closure,
            action,
            effect_class,
            authority,
            program,
            args,
            expected_effects,
            rollback_semantics,
        },
        readiness_evidence,
        authorization_evidence,
    })
}

struct LineCursor<'a> {
    lines: Vec<&'a str>,
    position: usize,
}

impl<'a> LineCursor<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            lines: text.split('\n').collect(),
            position: 0,
        }
    }

    fn next_line(&mut self) -> Result<&'a str, PreparedActivationRequestStoreError> {
        let line = self.lines.get(self.position).copied().ok_or_else(|| {
            PreparedActivationRequestStoreError::Malformed("unexpected end of request".into())
        })?;
        self.position += 1;
        Ok(line)
    }

    fn expect_literal(
        &mut self,
        expected: &str,
    ) -> Result<(), PreparedActivationRequestStoreError> {
        let observed = self.next_line()?;
        if observed != expected {
            return Err(PreparedActivationRequestStoreError::Malformed(format!(
                "expected {expected:?}, observed {observed:?}"
            )));
        }
        Ok(())
    }

    fn field(&mut self, key: &str) -> Result<&'a str, PreparedActivationRequestStoreError> {
        let line = self.next_line()?;
        let prefix = format!("{key}=");
        line.strip_prefix(&prefix).ok_or_else(|| {
            PreparedActivationRequestStoreError::Malformed(format!(
                "expected field {key}, observed {line:?}"
            ))
        })
    }

    fn hex_field(&mut self, key: &str) -> Result<String, PreparedActivationRequestStoreError> {
        decode_hex(self.field(key)?)
    }

    fn u64_field(&mut self, key: &str) -> Result<u64, PreparedActivationRequestStoreError> {
        self.field(key)?.parse::<u64>().map_err(|_| {
            PreparedActivationRequestStoreError::Malformed(format!("invalid u64 field {key}"))
        })
    }

    fn count_field(
        &mut self,
        key: &str,
        maximum: usize,
    ) -> Result<usize, PreparedActivationRequestStoreError> {
        let value = self.field(key)?.parse::<usize>().map_err(|_| {
            PreparedActivationRequestStoreError::Malformed(format!("invalid count field {key}"))
        })?;
        if value > maximum {
            return Err(PreparedActivationRequestStoreError::Malformed(format!(
                "count field {key} exceeds limit"
            )));
        }
        Ok(value)
    }

    fn finish(&mut self) -> Result<(), PreparedActivationRequestStoreError> {
        if !self.next_line()?.is_empty() || self.position != self.lines.len() {
            return Err(PreparedActivationRequestStoreError::Malformed(
                "trailing or missing request data".into(),
            ));
        }
        Ok(())
    }
}

fn parse_action(value: &str) -> Result<SystemCandidateAction, PreparedActivationRequestStoreError> {
    match value {
        "preview-activation" => Ok(SystemCandidateAction::PreviewActivation),
        "test-activation" => Ok(SystemCandidateAction::TestActivation),
        _ => Err(PreparedActivationRequestStoreError::Malformed(
            "unsupported prepared activation action".into(),
        )),
    }
}

fn parse_effect(value: &str) -> Result<SystemEffectClass, PreparedActivationRequestStoreError> {
    match value {
        "preview-hooks" => Ok(SystemEffectClass::PreviewHooks),
        "temporary-live-activation" => Ok(SystemEffectClass::TemporaryLiveActivation),
        _ => Err(PreparedActivationRequestStoreError::Malformed(
            "unsupported prepared activation effect class".into(),
        )),
    }
}

fn parse_authority(
    value: &str,
) -> Result<SystemAuthorityClass, PreparedActivationRequestStoreError> {
    match value {
        "host-administrator" => Ok(SystemAuthorityClass::HostAdministrator),
        _ => Err(PreparedActivationRequestStoreError::Malformed(
            "unsupported prepared activation authority".into(),
        )),
    }
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

fn validate_directory(
    path: &Path,
    expected_owner_uid: u32,
) -> Result<(), PreparedActivationRequestStoreError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| PreparedActivationRequestStoreError::Io(error.to_string()))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != expected_owner_uid
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(PreparedActivationRequestStoreError::InvalidLayout);
    }
    Ok(())
}

fn path_exists(path: &Path) -> Result<bool, PreparedActivationRequestStoreError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(PreparedActivationRequestStoreError::Io(error.to_string())),
    }
}

fn sync_dir(path: &Path) -> Result<(), PreparedActivationRequestStoreError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| PreparedActivationRequestStoreError::Io(error.to_string()))
}

fn hex_text(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn decode_hex(value: &str) -> Result<String, PreparedActivationRequestStoreError> {
    if value.len() % 2 != 0 {
        return Err(PreparedActivationRequestStoreError::Malformed(
            "odd-length hex field".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        bytes.push((hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?);
    }
    String::from_utf8(bytes).map_err(|_| {
        PreparedActivationRequestStoreError::Malformed("hex field is not UTF-8".into())
    })
}

fn hex_nibble(value: u8) -> Result<u8, PreparedActivationRequestStoreError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(PreparedActivationRequestStoreError::Malformed(
            "invalid lowercase hex field".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(1);
    const CANDIDATE: &str = "/nix/store/candidate-nixos-system-blob-pilot";

    fn prepared(
        authorization: &str,
        action: SystemCandidateAction,
    ) -> PreparedPrivilegedActivation {
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
            authorization: SystemAuthorizationId::from(authorization),
            authorization_expires_at_unix_ms: 61_000,
            prepared_at_unix_ms: 2_000,
            plan: ImmutableNixOsActivationPlan {
                operation_id: SystemOperationId::from("op:activate"),
                candidate: SystemCandidateId::from("candidate:one"),
                system_spec: SystemSpecId::from("system:one"),
                materialization_operation: SystemOperationId::from("op:materialize"),
                system_closure: CANDIDATE.into(),
                action,
                effect_class,
                authority: SystemAuthorityClass::HostAdministrator,
                program: format!("{CANDIDATE}/bin/switch-to-configuration"),
                args: vec![argument.into()],
                expected_effects: vec!["exact immutable activation".into()],
                rollback_semantics: "reboot restores baseline".into(),
            },
            readiness_evidence: vec![
                "node:node:lab".into(),
                "observed-at-unix-ms:1000".into(),
            ],
            authorization_evidence: vec![
                format!("authorization:{authorization}"),
                "expires-at-unix-ms:61000".into(),
            ],
        }
    }

    struct TempStore {
        root: PathBuf,
        uid: u32,
    }

    impl TempStore {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "theblob-prepared-request-store-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            for state in [READY, INFLIGHT, COMPLETED, FAILED] {
                let path = root.join(state);
                fs::create_dir(&path).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let uid = fs::symlink_metadata(&root).unwrap().uid();
            Self { root, uid }
        }

        fn store(&self) -> FilePreparedActivationRequestStore {
            FilePreparedActivationRequestStore::new(&self.root, self.uid)
        }

        fn stage(&self, prepared: &PreparedPrivilegedActivation) {
            let path = self.root.join(READY).join(format!(
                "authorization-{}.request",
                hex_text(prepared.authorization.as_str())
            ));
            fs::write(&path, canonical_text(prepared)).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
    }

    impl Drop for TempStore {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn canonical_request_round_trips_exactly() {
        let request = prepared("auth:round-trip", SystemCandidateAction::TestActivation);
        let text = canonical_text(&request);
        assert_eq!(parse_canonical_text(&text), Ok(request));
    }

    #[test]
    fn parser_rejects_trailing_fields() {
        let request = prepared("auth:trailing", SystemCandidateAction::PreviewActivation);
        let mut text = canonical_text(&request);
        text.push_str("unexpected=value\n");
        assert!(matches!(
            parse_canonical_text(&text),
            Err(PreparedActivationRequestStoreError::Malformed(_))
        ));
    }

    #[test]
    fn claim_is_durable_and_not_reopened_after_recovery() {
        let temp = TempStore::new();
        let request = prepared("auth:crash", SystemCandidateAction::PreviewActivation);
        temp.stage(&request);
        let store = temp.store();
        assert_eq!(store.load_ready(&request.authorization), Ok(request.clone()));
        let claimed = store.claim_exact(&request).expect("claim request");
        drop(store);

        let recovered = temp.store();
        assert_eq!(
            recovered.load_ready(&request.authorization),
            Err(PreparedActivationRequestStoreError::AlreadyClaimed(
                request.authorization.clone()
            ))
        );
        recovered.mark_failed(&claimed).expect("quarantine failed request");
        assert!(matches!(
            recovered.load_ready(&request.authorization),
            Err(PreparedActivationRequestStoreError::TerminalState(_))
        ));
    }

    #[test]
    fn successful_completion_is_terminal() {
        let temp = TempStore::new();
        let request = prepared("auth:complete", SystemCandidateAction::TestActivation);
        temp.stage(&request);
        let store = temp.store();
        let claimed = store.claim_exact(&request).expect("claim request");
        store.mark_completed(&claimed).expect("complete request");
        assert!(matches!(
            store.load_ready(&request.authorization),
            Err(PreparedActivationRequestStoreError::TerminalState(_))
        ));
    }

    #[test]
    fn permissive_directory_mode_fails_closed() {
        let temp = TempStore::new();
        let request = prepared("auth:mode", SystemCandidateAction::PreviewActivation);
        temp.stage(&request);
        fs::set_permissions(temp.root.join(READY), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            temp.store().load_ready(&request.authorization),
            Err(PreparedActivationRequestStoreError::InvalidLayout)
        );
    }
}
