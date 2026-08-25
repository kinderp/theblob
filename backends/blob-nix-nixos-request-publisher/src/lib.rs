#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use blob_core::{
    NodeId, PhysicalTestNodeProfile, PhysicalTestNodeReadiness, PhysicalTestNodeViolation,
    SystemArchitecture, SystemAuthorizationId, SystemCandidateAction, SystemCandidateId,
    SystemOperationId, SystemSpecId,
};
use blob_nix_nixos_activation::{
    ImmutableActivationError, ImmutableNixOsActivationPlanner, MaterializedNixOsCandidate,
};
use blob_nix_nixos_authority::{
    PkcheckAuthorizationChecker, PolkitAuthorizationError, PolkitAuthorizationRequest,
    StdPkcheckCommandRunner,
};
use blob_nix_nixos_request_store::{canonical_text, DEFAULT_PREPARED_REQUEST_ROOT};
use blob_node_probe::{NixOsProbeError, NixOsReadOnlyProbe, NodeSafetyConfirmations};
use blob_system_activation_gate::PreparedPrivilegedActivation;

pub const DEFAULT_MATERIALIZATION_ADMISSION_ROOT: &str =
    "/var/lib/theblob/materialization-admissions";
pub const DEFAULT_PUBLISHED_AUTHORIZATION_TTL_MS: u64 = 120_000;
pub const MAX_MATERIALIZATION_ADMISSION_BYTES: u64 = 16 * 1024;

const READY: &str = "ready";
const INFLIGHT: &str = "inflight";
const COMPLETED: &str = "completed";
const FAILED: &str = "failed";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterializationAdmission {
    pub node: NodeId,
    pub candidate: SystemCandidateId,
    pub system_spec: SystemSpecId,
    pub materialization_operation: SystemOperationId,
    pub system_closure: String,
    pub admitted_at_unix_ms: u64,
    pub provenance: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MaterializationAdmissionError {
    Missing(SystemOperationId),
    InvalidRoot,
    InvalidFile,
    TooLarge,
    Malformed(String),
    NonCanonical,
    OperationMismatch,
    InvalidSystemClosure,
    Io(String),
}

pub struct FileMaterializationAdmissionStore {
    root: PathBuf,
    expected_owner_uid: u32,
}

impl FileMaterializationAdmissionStore {
    pub fn production_default() -> Self {
        Self::new(DEFAULT_MATERIALIZATION_ADMISSION_ROOT, 0)
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

    pub fn load(
        &self,
        operation: &SystemOperationId,
    ) -> Result<MaterializationAdmission, MaterializationAdmissionError> {
        validate_root_directory(&self.root, self.expected_owner_uid)
            .map_err(|_| MaterializationAdmissionError::InvalidRoot)?;
        let path = self.path(operation);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                MaterializationAdmissionError::Missing(operation.clone())
            } else {
                MaterializationAdmissionError::Io(error.to_string())
            }
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != self.expected_owner_uid
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(MaterializationAdmissionError::InvalidFile);
        }
        if metadata.len() > MAX_MATERIALIZATION_ADMISSION_BYTES {
            return Err(MaterializationAdmissionError::TooLarge);
        }

        let mut text = String::new();
        File::open(&path)
            .and_then(|mut file| file.read_to_string(&mut text))
            .map_err(|error| MaterializationAdmissionError::Io(error.to_string()))?;
        let admission = parse_materialization_admission(&text)?;
        if admission.materialization_operation != *operation {
            return Err(MaterializationAdmissionError::OperationMismatch);
        }
        if canonical_materialization_admission(&admission) != text {
            return Err(MaterializationAdmissionError::NonCanonical);
        }
        validate_store_closure(&admission.system_closure)?;
        Ok(admission)
    }

    pub fn path(&self, operation: &SystemOperationId) -> PathBuf {
        self.root.join(format!(
            "operation-{}.admission",
            hex_text(operation.as_str())
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedPreparedActivation {
    pub authorization: SystemAuthorizationId,
    pub authorized_system_bus_name: String,
    pub request_path: PathBuf,
    pub prepared: PreparedPrivilegedActivation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreparedRequestPublishError {
    Admission(MaterializationAdmissionError),
    AdmissionNodeMismatch,
    Probe(NixOsProbeError),
    ReadinessRejected(Vec<PhysicalTestNodeViolation>),
    Plan(ImmutableActivationError),
    Polkit(PolkitAuthorizationError),
    ClockOverflow,
    RandomSource(String),
    InvalidRequestRoot,
    RequestAlreadyExists(SystemAuthorizationId),
    InvalidCreatedRequest,
    Io(String),
}

pub struct RootPreparedActivationPublisher {
    local_node: NodeId,
    profile: PhysicalTestNodeProfile,
    confirmations: NodeSafetyConfirmations,
    admissions: FileMaterializationAdmissionStore,
    request_root: PathBuf,
    expected_owner_uid: u32,
    authorization_ttl_ms: u64,
}

impl RootPreparedActivationPublisher {
    pub fn production_default(
        local_node: impl Into<NodeId>,
        architecture: SystemArchitecture,
        confirmations: NodeSafetyConfirmations,
    ) -> Self {
        let local_node = local_node.into();
        Self {
            profile: PhysicalTestNodeProfile::nixos_pilot(local_node.clone(), architecture),
            local_node,
            confirmations,
            admissions: FileMaterializationAdmissionStore::production_default(),
            request_root: PathBuf::from(DEFAULT_PREPARED_REQUEST_ROOT),
            expected_owner_uid: 0,
            authorization_ttl_ms: DEFAULT_PUBLISHED_AUTHORIZATION_TTL_MS,
        }
    }

    pub fn new(
        local_node: impl Into<NodeId>,
        profile: PhysicalTestNodeProfile,
        confirmations: NodeSafetyConfirmations,
        admission_root: impl Into<PathBuf>,
        request_root: impl Into<PathBuf>,
        expected_owner_uid: u32,
        authorization_ttl_ms: u64,
    ) -> Self {
        let local_node = local_node.into();
        Self {
            local_node,
            profile,
            confirmations,
            admissions: FileMaterializationAdmissionStore::new(
                admission_root,
                expected_owner_uid,
            ),
            request_root: request_root.into(),
            expected_owner_uid,
            authorization_ttl_ms,
        }
    }

    /// Build and publish one exact request from trusted local inputs.
    ///
    /// The caller supplies only a live system-bus sender, the id of a root-owned
    /// materialization admission, and preview/test intent. Closure, candidate,
    /// SystemSpec, readiness, executable and argv are never taken from the caller.
    /// A real polkit grant is required before the root-owned ready file is created.
    pub fn publish_user_initiated(
        &self,
        system_bus_name: impl Into<String>,
        materialization_operation: &SystemOperationId,
        action: SystemCandidateAction,
        pkcheck_program: impl Into<PathBuf>,
    ) -> Result<PublishedPreparedActivation, PreparedRequestPublishError> {
        let admission = self
            .admissions
            .load(materialization_operation)
            .map_err(PreparedRequestPublishError::Admission)?;
        if admission.node != self.local_node {
            return Err(PreparedRequestPublishError::AdmissionNodeMismatch);
        }

        let snapshot = NixOsReadOnlyProbe::observe_current_host(self.local_node.clone())
            .map_err(PreparedRequestPublishError::Probe)?;
        let readiness = snapshot.to_readiness(&self.confirmations);
        self.profile
            .validate_readiness(&action, &readiness)
            .map_err(PreparedRequestPublishError::ReadinessRejected)?;

        let prepared = self.build_prepared(&admission, action, &readiness)?;
        let request = PolkitAuthorizationRequest::for_prepared(system_bus_name, &prepared)
            .map_err(PreparedRequestPublishError::Polkit)?;
        let checker = PkcheckAuthorizationChecker::new(pkcheck_program, StdPkcheckCommandRunner)
            .map_err(PreparedRequestPublishError::Polkit)?;
        let grant = checker
            .check_user_initiated(&request, prepared.prepared_at_unix_ms)
            .map_err(PreparedRequestPublishError::Polkit)?;

        let path = self.publish_ready(&prepared)?;
        Ok(PublishedPreparedActivation {
            authorization: prepared.authorization.clone(),
            authorized_system_bus_name: grant.system_bus_name().to_owned(),
            request_path: path,
            prepared,
        })
    }

    fn build_prepared(
        &self,
        admission: &MaterializationAdmission,
        action: SystemCandidateAction,
        readiness: &PhysicalTestNodeReadiness,
    ) -> Result<PreparedPrivilegedActivation, PreparedRequestPublishError> {
        let nonce = random_hex_128().map_err(PreparedRequestPublishError::RandomSource)?;
        let authorization = SystemAuthorizationId::from(format!("auth:published-{nonce}"));
        let operation = blob_core::SystemCandidateOperation::new(
            format!("op:published-{nonce}"),
            admission.candidate.clone(),
            admission.system_spec.clone(),
            action,
        );
        let materialized = MaterializedNixOsCandidate {
            candidate: admission.candidate.clone(),
            system_spec: admission.system_spec.clone(),
            materialization_operation: admission.materialization_operation.clone(),
            system_closure: admission.system_closure.clone(),
        };
        let plan = ImmutableNixOsActivationPlanner::plan(&operation, &materialized)
            .map_err(PreparedRequestPublishError::Plan)?;

        let prepared_at_unix_ms = readiness.observed_at_unix_ms;
        let authorization_expires_at_unix_ms = prepared_at_unix_ms
            .checked_add(self.authorization_ttl_ms)
            .ok_or(PreparedRequestPublishError::ClockOverflow)?;
        let mut readiness_evidence = readiness.evidence_lines();
        readiness_evidence.push(format!(
            "materialization-admission:{}",
            admission.materialization_operation
        ));
        let authorization_evidence = vec![
            format!("authorization:{authorization}"),
            format!("expires-at-unix-ms:{authorization_expires_at_unix_ms}"),
            "grant-stage:root-request-publisher-polkit".into(),
        ];

        Ok(PreparedPrivilegedActivation {
            node: readiness.node.clone(),
            readiness_observed_at_unix_ms: readiness.observed_at_unix_ms,
            authorization,
            authorization_expires_at_unix_ms,
            prepared_at_unix_ms,
            plan,
            readiness_evidence,
            authorization_evidence,
        })
    }

    fn publish_ready(
        &self,
        prepared: &PreparedPrivilegedActivation,
    ) -> Result<PathBuf, PreparedRequestPublishError> {
        validate_root_directory(&self.request_root, self.expected_owner_uid)
            .map_err(|_| PreparedRequestPublishError::InvalidRequestRoot)?;
        for state in [READY, INFLIGHT, COMPLETED, FAILED] {
            validate_root_directory(&self.request_root.join(state), self.expected_owner_uid)
                .map_err(|_| PreparedRequestPublishError::InvalidRequestRoot)?;
        }

        for state in [READY, INFLIGHT, COMPLETED, FAILED] {
            for suffix in ["request", "claim"] {
                let path = self.state_path(state, &prepared.authorization, suffix);
                if path_exists(&path).map_err(PreparedRequestPublishError::Io)? {
                    return Err(PreparedRequestPublishError::RequestAlreadyExists(
                        prepared.authorization.clone(),
                    ));
                }
            }
        }

        let path = self.state_path(READY, &prepared.authorization, "request");
        let text = canonical_text(prepared);
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(PreparedRequestPublishError::RequestAlreadyExists(
                    prepared.authorization.clone(),
                ));
            }
            Err(error) => return Err(PreparedRequestPublishError::Io(error.to_string())),
        };
        if let Err(error) = file
            .write_all(text.as_bytes())
            .and_then(|_| file.sync_all())
        {
            let _ = fs::remove_file(&path);
            return Err(PreparedRequestPublishError::Io(error.to_string()));
        }

        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| PreparedRequestPublishError::Io(error.to_string()))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != self.expected_owner_uid
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            let _ = fs::remove_file(&path);
            return Err(PreparedRequestPublishError::InvalidCreatedRequest);
        }
        sync_dir(&self.request_root.join(READY)).map_err(PreparedRequestPublishError::Io)?;
        Ok(path)
    }

    fn state_path(
        &self,
        state: &str,
        authorization: &SystemAuthorizationId,
        suffix: &str,
    ) -> PathBuf {
        self.request_root.join(state).join(format!(
            "authorization-{}.{}",
            hex_text(authorization.as_str()),
            suffix
        ))
    }
}

pub fn canonical_materialization_admission(admission: &MaterializationAdmission) -> String {
    let mut lines = vec![
        "theblob-materialization-admission-v1".to_owned(),
        format!(
            "materialization-operation={}",
            hex_text(admission.materialization_operation.as_str())
        ),
        format!("node={}", hex_text(admission.node.as_str())),
        format!("candidate={}", hex_text(admission.candidate.as_str())),
        format!("system-spec={}", hex_text(admission.system_spec.as_str())),
        format!("system-closure={}", hex_text(&admission.system_closure)),
        format!("admitted-at-unix-ms={}", admission.admitted_at_unix_ms),
        format!("provenance-count={}", admission.provenance.len()),
    ];
    for (index, value) in admission.provenance.iter().enumerate() {
        lines.push(format!("provenance-{index}={}", hex_text(value)));
    }
    lines.push(String::new());
    lines.join("\n")
}

pub fn parse_materialization_admission(
    text: &str,
) -> Result<MaterializationAdmission, MaterializationAdmissionError> {
    if text.len() as u64 > MAX_MATERIALIZATION_ADMISSION_BYTES {
        return Err(MaterializationAdmissionError::TooLarge);
    }
    let mut cursor = AdmissionCursor::new(text);
    cursor.literal("theblob-materialization-admission-v1")?;
    let materialization_operation =
        SystemOperationId::from(cursor.hex_field("materialization-operation")?);
    let node = NodeId::from(cursor.hex_field("node")?);
    let candidate = SystemCandidateId::from(cursor.hex_field("candidate")?);
    let system_spec = SystemSpecId::from(cursor.hex_field("system-spec")?);
    let system_closure = cursor.hex_field("system-closure")?;
    let admitted_at_unix_ms = cursor.u64_field("admitted-at-unix-ms")?;
    let provenance_count = cursor.count_field("provenance-count", 128)?;
    let mut provenance = Vec::with_capacity(provenance_count);
    for index in 0..provenance_count {
        provenance.push(cursor.hex_field(&format!("provenance-{index}"))?);
    }
    cursor.finish()?;
    validate_store_closure(&system_closure)?;
    Ok(MaterializationAdmission {
        node,
        candidate,
        system_spec,
        materialization_operation,
        system_closure,
        admitted_at_unix_ms,
        provenance,
    })
}

struct AdmissionCursor<'a> {
    lines: Vec<&'a str>,
    position: usize,
}

impl<'a> AdmissionCursor<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            lines: text.split('\n').collect(),
            position: 0,
        }
    }

    fn next(&mut self) -> Result<&'a str, MaterializationAdmissionError> {
        let value = self.lines.get(self.position).copied().ok_or_else(|| {
            MaterializationAdmissionError::Malformed("unexpected end of admission".into())
        })?;
        self.position += 1;
        Ok(value)
    }

    fn literal(&mut self, expected: &str) -> Result<(), MaterializationAdmissionError> {
        let observed = self.next()?;
        if observed != expected {
            return Err(MaterializationAdmissionError::Malformed(format!(
                "expected {expected:?}, observed {observed:?}"
            )));
        }
        Ok(())
    }

    fn field(&mut self, key: &str) -> Result<&'a str, MaterializationAdmissionError> {
        let line = self.next()?;
        let prefix = format!("{key}=");
        line.strip_prefix(&prefix).ok_or_else(|| {
            MaterializationAdmissionError::Malformed(format!(
                "expected field {key}, observed {line:?}"
            ))
        })
    }

    fn hex_field(&mut self, key: &str) -> Result<String, MaterializationAdmissionError> {
        decode_hex(self.field(key)?)
    }

    fn u64_field(&mut self, key: &str) -> Result<u64, MaterializationAdmissionError> {
        self.field(key)?.parse::<u64>().map_err(|_| {
            MaterializationAdmissionError::Malformed(format!("invalid u64 field {key}"))
        })
    }

    fn count_field(
        &mut self,
        key: &str,
        maximum: usize,
    ) -> Result<usize, MaterializationAdmissionError> {
        let value = self.field(key)?.parse::<usize>().map_err(|_| {
            MaterializationAdmissionError::Malformed(format!("invalid count field {key}"))
        })?;
        if value > maximum {
            return Err(MaterializationAdmissionError::Malformed(format!(
                "count field {key} exceeds limit"
            )));
        }
        Ok(value)
    }

    fn finish(&mut self) -> Result<(), MaterializationAdmissionError> {
        if !self.next()?.is_empty() || self.position != self.lines.len() {
            return Err(MaterializationAdmissionError::Malformed(
                "trailing or missing admission data".into(),
            ));
        }
        Ok(())
    }
}

fn validate_store_closure(value: &str) -> Result<(), MaterializationAdmissionError> {
    let components = Path::new(value).components().collect::<Vec<_>>();
    if matches!(
        components.as_slice(),
        [
            Component::RootDir,
            Component::Normal(nix),
            Component::Normal(store),
            Component::Normal(closure)
        ] if *nix == "nix" && *store == "store" && !closure.is_empty()
    ) {
        Ok(())
    } else {
        Err(MaterializationAdmissionError::InvalidSystemClosure)
    }
}

fn validate_root_directory(path: &Path, expected_owner_uid: u32) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != expected_owner_uid
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("invalid root-owned directory".into());
    }
    Ok(())
}

fn path_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn sync_dir(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

fn random_hex_128() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| error.to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn hex_text(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn decode_hex(value: &str) -> Result<String, MaterializationAdmissionError> {
    if value.len() % 2 != 0 {
        return Err(MaterializationAdmissionError::Malformed(
            "odd-length hex field".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        bytes.push((hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?);
    }
    String::from_utf8(bytes).map_err(|_| {
        MaterializationAdmissionError::Malformed("hex field is not UTF-8".into())
    })
}

fn hex_nibble(value: u8) -> Result<u8, MaterializationAdmissionError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(MaterializationAdmissionError::Malformed(
            "invalid lowercase hex field".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialization_admission_round_trips_exactly() {
        let admission = MaterializationAdmission {
            node: NodeId::from("node:lab"),
            candidate: SystemCandidateId::from("candidate:one"),
            system_spec: SystemSpecId::from("system:one"),
            materialization_operation: SystemOperationId::from("op:materialize"),
            system_closure: "/nix/store/abc-system".into(),
            admitted_at_unix_ms: 42,
            provenance: vec!["materialization-result:verified".into()],
        };
        let text = canonical_materialization_admission(&admission);
        assert_eq!(parse_materialization_admission(&text), Ok(admission));
    }

    #[test]
    fn admission_rejects_non_store_closure() {
        let text = [
            "theblob-materialization-admission-v1",
            "materialization-operation=6f703a6d6174657269616c697a65",
            "node=6e6f64653a6c6162",
            "candidate=63616e6469646174653a6f6e65",
            "system-spec=73797374656d3a6f6e65",
            "system-closure=2f746d702f6576696c",
            "admitted-at-unix-ms=42",
            "provenance-count=0",
            "",
        ]
        .join("\n");
        assert_eq!(
            parse_materialization_admission(&text),
            Err(MaterializationAdmissionError::InvalidSystemClosure)
        );
    }
}
