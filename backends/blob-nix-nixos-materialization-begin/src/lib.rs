#![forbid(unsafe_code)]

use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use blob_core::{NodeId, SystemCandidateId, SystemOperationId, SystemSpecId};
use blob_nix_nixos_materialization_authority::{
    MaterializationAuthorityError, MaterializationIntent, MaterializationIntentSpec,
    NixMaterializationInspector, RootMaterializationAdmissionAuthority,
};
use blob_nix_nixos_request_publisher::MaterializationAdmission;

pub const DEFAULT_TRUSTED_CANDIDATE_ROOT: &str =
    "/var/lib/theblob/materialization-candidates";
pub const DEFAULT_MATERIALIZATION_INTENT_ROOT: &str =
    "/var/lib/theblob/materialization-intents";
pub const DEFAULT_MATERIALIZATION_ADMISSION_ROOT: &str =
    "/var/lib/theblob/materialization-admissions";
pub const DEFAULT_PENDING_GCROOT_ROOT: &str =
    "/nix/var/nix/gcroots/theblob-materializations";
pub const MAX_TRUSTED_CANDIDATE_BYTES: u64 = 32 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedMaterializationCandidate {
    pub manifest_id: String,
    pub candidate: SystemCandidateId,
    pub system_spec: SystemSpecId,
    pub immutable_flake_root: PathBuf,
    pub installable_attribute: String,
    pub provenance: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrustedCandidateError {
    Missing(String),
    InvalidRoot,
    InvalidFile,
    TooLarge,
    Malformed,
    NonCanonical,
    ManifestMismatch,
    InvalidManifestId,
    InvalidImmutableFlakeRoot,
    InvalidInstallableAttribute,
    Io(String),
}

pub struct FileTrustedMaterializationCandidateStore {
    root: PathBuf,
    expected_owner_uid: u32,
}

impl FileTrustedMaterializationCandidateStore {
    pub fn production_default() -> Self {
        Self::new(DEFAULT_TRUSTED_CANDIDATE_ROOT, 0)
    }

    pub fn new(root: impl Into<PathBuf>, expected_owner_uid: u32) -> Self {
        Self {
            root: root.into(),
            expected_owner_uid,
        }
    }

    pub fn path(&self, manifest_id: &str) -> PathBuf {
        self.root
            .join(format!("manifest-{}.candidate", hex_text(manifest_id)))
    }

    pub fn load(
        &self,
        manifest_id: &str,
    ) -> Result<TrustedMaterializationCandidate, TrustedCandidateError> {
        validate_manifest_id(manifest_id).map_err(|_| TrustedCandidateError::InvalidManifestId)?;
        validate_directory(&self.root, self.expected_owner_uid)
            .map_err(|_| TrustedCandidateError::InvalidRoot)?;

        let path = self.path(manifest_id);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                TrustedCandidateError::Missing(manifest_id.to_owned())
            } else {
                TrustedCandidateError::Io(error.to_string())
            }
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != self.expected_owner_uid
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(TrustedCandidateError::InvalidFile);
        }
        if metadata.len() > MAX_TRUSTED_CANDIDATE_BYTES {
            return Err(TrustedCandidateError::TooLarge);
        }

        let mut text = String::new();
        File::open(&path)
            .and_then(|mut file| file.read_to_string(&mut text))
            .map_err(|error| TrustedCandidateError::Io(error.to_string()))?;
        let candidate = parse_trusted_candidate(&text)?;
        if candidate.manifest_id != manifest_id {
            return Err(TrustedCandidateError::ManifestMismatch);
        }
        if canonical_trusted_candidate(&candidate) != text {
            return Err(TrustedCandidateError::NonCanonical);
        }
        Ok(candidate)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MaterializationBeginError {
    Candidate(TrustedCandidateError),
    Authority(MaterializationAuthorityError),
    Inspector(String),
    InvalidGcRoot,
    GcRootConflict,
    IdentityChanged,
    Clock(String),
    RandomSource(String),
    Io(String),
}

pub struct RootMaterializationBeginBoundary {
    local_node: NodeId,
    candidates: FileTrustedMaterializationCandidateStore,
    authority: RootMaterializationAdmissionAuthority,
    gcroot_root: PathBuf,
    expected_owner_uid: u32,
}

impl RootMaterializationBeginBoundary {
    pub fn production_default(local_node: impl Into<NodeId>) -> Self {
        Self::new(
            local_node,
            DEFAULT_TRUSTED_CANDIDATE_ROOT,
            DEFAULT_MATERIALIZATION_INTENT_ROOT,
            DEFAULT_MATERIALIZATION_ADMISSION_ROOT,
            DEFAULT_PENDING_GCROOT_ROOT,
            0,
        )
    }

    pub fn new(
        local_node: impl Into<NodeId>,
        candidate_root: impl Into<PathBuf>,
        intent_root: impl Into<PathBuf>,
        admission_root: impl Into<PathBuf>,
        gcroot_root: impl Into<PathBuf>,
        expected_owner_uid: u32,
    ) -> Self {
        Self {
            local_node: local_node.into(),
            candidates: FileTrustedMaterializationCandidateStore::new(
                candidate_root,
                expected_owner_uid,
            ),
            authority: RootMaterializationAdmissionAuthority::new(
                intent_root,
                admission_root,
                expected_owner_uid,
            ),
            gcroot_root: gcroot_root.into(),
            expected_owner_uid,
        }
    }

    /// Begin from one already-trusted candidate manifest.
    ///
    /// The caller selects only the opaque manifest id. Candidate identity,
    /// SystemSpec identity, immutable source and installable attribute all come
    /// from the root-owned manifest. The materialization operation id and time
    /// are generated inside this root boundary.
    pub fn begin<I: NixMaterializationInspector>(
        &self,
        manifest_id: &str,
        inspector: &I,
    ) -> Result<MaterializationIntent, MaterializationBeginError> {
        let candidate = self
            .candidates
            .load(manifest_id)
            .map_err(MaterializationBeginError::Candidate)?;
        validate_canonical_immutable_store_subpath(&candidate.immutable_flake_root)
            .map_err(|_| MaterializationBeginError::Candidate(
                TrustedCandidateError::InvalidImmutableFlakeRoot,
            ))?;

        let resolved = inspector
            .resolve_exact_derivation(
                &candidate.immutable_flake_root,
                &candidate.installable_attribute,
            )
            .map_err(MaterializationBeginError::Inspector)?;
        let operation = SystemOperationId::from(format!(
            "op:materialize-{}",
            random_hex_128().map_err(MaterializationBeginError::RandomSource)?
        ));

        self.retain_derivation(&operation, &resolved.derivation_path)?;
        let intent = match self.authority.begin(
            &MaterializationIntentSpec {
                node: self.local_node.clone(),
                candidate: candidate.candidate,
                system_spec: candidate.system_spec,
                materialization_operation: operation.clone(),
                immutable_flake_root: candidate.immutable_flake_root,
                installable_attribute: candidate.installable_attribute,
                created_at_unix_ms: now_unix_ms().map_err(MaterializationBeginError::Clock)?,
            },
            inspector,
        ) {
            Ok(intent) => intent,
            Err(error) => {
                let _ = self.release_derivation(&operation);
                return Err(MaterializationBeginError::Authority(error));
            }
        };

        if intent.derivation_path != resolved.derivation_path
            || intent.expected_output != resolved.expected_output
        {
            // Keep the GC root when a pending intent was already persisted. That
            // fails safe: recovery can inspect the durable mismatch instead of
            // leaving a live intent whose committed derivation was collected.
            return Err(MaterializationBeginError::IdentityChanged);
        }
        Ok(intent)
    }

    pub fn resume(
        &self,
        operation: &SystemOperationId,
    ) -> Result<MaterializationIntent, MaterializationBeginError> {
        let intent = self
            .authority
            .load_pending(operation)
            .map_err(MaterializationBeginError::Authority)?;
        self.require_derivation(operation, &intent.derivation_path)?;
        Ok(intent)
    }

    pub fn complete<I: NixMaterializationInspector>(
        &self,
        operation: &SystemOperationId,
        inspector: &I,
    ) -> Result<MaterializationAdmission, MaterializationBeginError> {
        let pending = self
            .authority
            .load_pending(operation)
            .map_err(MaterializationBeginError::Authority)?;
        self.require_derivation(operation, &pending.derivation_path)?;
        let admission = self
            .authority
            .complete(
                operation,
                now_unix_ms().map_err(MaterializationBeginError::Clock)?,
                inspector,
            )
            .map_err(MaterializationBeginError::Authority)?;
        if admission.system_closure != pending.expected_output.display().to_string() {
            return Err(MaterializationBeginError::IdentityChanged);
        }
        self.release_derivation(operation)?;
        Ok(admission)
    }

    fn gcroot_path(&self, operation: &SystemOperationId) -> PathBuf {
        self.gcroot_root.join(format!(
            "operation-{}-derivation",
            hex_text(operation.as_str())
        ))
    }

    fn validate_gcroot_root(&self) -> Result<(), MaterializationBeginError> {
        validate_directory(&self.gcroot_root, self.expected_owner_uid)
            .map_err(|_| MaterializationBeginError::InvalidGcRoot)
    }

    fn retain_derivation(
        &self,
        operation: &SystemOperationId,
        derivation: &Path,
    ) -> Result<(), MaterializationBeginError> {
        self.validate_gcroot_root()?;
        let root = self.gcroot_path(operation);
        match fs::symlink_metadata(&root) {
            Ok(metadata) => {
                if !metadata.file_type().is_symlink() {
                    return Err(MaterializationBeginError::GcRootConflict);
                }
                let target = fs::read_link(&root)
                    .map_err(|error| MaterializationBeginError::Io(error.to_string()))?;
                if target != derivation {
                    return Err(MaterializationBeginError::GcRootConflict);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                symlink(derivation, &root)
                    .map_err(|error| MaterializationBeginError::Io(error.to_string()))?;
                sync_dir(&self.gcroot_root)?;
            }
            Err(error) => return Err(MaterializationBeginError::Io(error.to_string())),
        }
        Ok(())
    }

    fn require_derivation(
        &self,
        operation: &SystemOperationId,
        derivation: &Path,
    ) -> Result<(), MaterializationBeginError> {
        self.validate_gcroot_root()?;
        let root = self.gcroot_path(operation);
        let metadata = fs::symlink_metadata(&root)
            .map_err(|error| MaterializationBeginError::Io(error.to_string()))?;
        if !metadata.file_type().is_symlink() {
            return Err(MaterializationBeginError::GcRootConflict);
        }
        let target = fs::read_link(&root)
            .map_err(|error| MaterializationBeginError::Io(error.to_string()))?;
        if target != derivation || !derivation.exists() {
            return Err(MaterializationBeginError::GcRootConflict);
        }
        Ok(())
    }

    fn release_derivation(
        &self,
        operation: &SystemOperationId,
    ) -> Result<(), MaterializationBeginError> {
        self.validate_gcroot_root()?;
        fs::remove_file(self.gcroot_path(operation))
            .map_err(|error| MaterializationBeginError::Io(error.to_string()))?;
        sync_dir(&self.gcroot_root)
    }
}

pub fn canonical_trusted_candidate(candidate: &TrustedMaterializationCandidate) -> String {
    let mut lines = vec![
        "theblob-trusted-materialization-candidate-v1".to_owned(),
        format!("manifest-id={}", hex_text(&candidate.manifest_id)),
        format!("candidate={}", hex_text(candidate.candidate.as_str())),
        format!("system-spec={}", hex_text(candidate.system_spec.as_str())),
        format!(
            "immutable-flake-root={}",
            hex_text(&candidate.immutable_flake_root.display().to_string())
        ),
        format!(
            "installable-attribute={}",
            hex_text(&candidate.installable_attribute)
        ),
        format!("provenance-count={}", candidate.provenance.len()),
    ];
    for (index, evidence) in candidate.provenance.iter().enumerate() {
        lines.push(format!("provenance-{index}={}", hex_text(evidence)));
    }
    lines.push(String::new());
    lines.join("\n")
}

pub fn parse_trusted_candidate(
    text: &str,
) -> Result<TrustedMaterializationCandidate, TrustedCandidateError> {
    if text.len() as u64 > MAX_TRUSTED_CANDIDATE_BYTES {
        return Err(TrustedCandidateError::TooLarge);
    }
    let mut cursor = Cursor::new(text);
    cursor.literal("theblob-trusted-materialization-candidate-v1")?;
    let manifest_id = cursor.hex_field("manifest-id")?;
    let candidate = SystemCandidateId::from(cursor.hex_field("candidate")?);
    let system_spec = SystemSpecId::from(cursor.hex_field("system-spec")?);
    let immutable_flake_root = PathBuf::from(cursor.hex_field("immutable-flake-root")?);
    let installable_attribute = cursor.hex_field("installable-attribute")?;
    let provenance_count = cursor.count_field("provenance-count", 128)?;
    let mut provenance = Vec::with_capacity(provenance_count);
    for index in 0..provenance_count {
        provenance.push(cursor.hex_field(&format!("provenance-{index}"))?);
    }
    cursor.finish()?;

    validate_manifest_id(&manifest_id).map_err(|_| TrustedCandidateError::InvalidManifestId)?;
    validate_immutable_store_subpath(&immutable_flake_root)
        .map_err(|_| TrustedCandidateError::InvalidImmutableFlakeRoot)?;
    validate_installable_attribute(&installable_attribute)
        .map_err(|_| TrustedCandidateError::InvalidInstallableAttribute)?;

    Ok(TrustedMaterializationCandidate {
        manifest_id,
        candidate,
        system_spec,
        immutable_flake_root,
        installable_attribute,
        provenance,
    })
}

struct Cursor<'a> {
    lines: Vec<&'a str>,
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            lines: text.split('\n').collect(),
            position: 0,
        }
    }

    fn next(&mut self) -> Result<&'a str, TrustedCandidateError> {
        let line = self
            .lines
            .get(self.position)
            .copied()
            .ok_or(TrustedCandidateError::Malformed)?;
        self.position += 1;
        Ok(line)
    }

    fn literal(&mut self, expected: &str) -> Result<(), TrustedCandidateError> {
        if self.next()? != expected {
            return Err(TrustedCandidateError::Malformed);
        }
        Ok(())
    }

    fn field(&mut self, key: &str) -> Result<&'a str, TrustedCandidateError> {
        self.next()?
            .strip_prefix(&format!("{key}="))
            .ok_or(TrustedCandidateError::Malformed)
    }

    fn hex_field(&mut self, key: &str) -> Result<String, TrustedCandidateError> {
        decode_hex(self.field(key)?)
    }

    fn count_field(&mut self, key: &str, maximum: usize) -> Result<usize, TrustedCandidateError> {
        let value = self
            .field(key)?
            .parse::<usize>()
            .map_err(|_| TrustedCandidateError::Malformed)?;
        if value > maximum {
            return Err(TrustedCandidateError::Malformed);
        }
        Ok(value)
    }

    fn finish(&mut self) -> Result<(), TrustedCandidateError> {
        if !self.next()?.is_empty() || self.position != self.lines.len() {
            return Err(TrustedCandidateError::Malformed);
        }
        Ok(())
    }
}

fn validate_manifest_id(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 256 {
        return Err("invalid manifest id length".into());
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_' | b'.'))
    {
        return Err("invalid manifest id".into());
    }
    Ok(())
}

fn validate_installable_attribute(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 512 {
        return Err("invalid installable attribute length".into());
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err("invalid installable attribute".into());
    }
    Ok(())
}

fn validate_immutable_store_subpath(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("source is not absolute".into());
    }
    let components = path.components().collect::<Vec<_>>();
    if components.len() < 4
        || components[0] != Component::RootDir
        || components[1] != Component::Normal("nix".as_ref())
        || components[2] != Component::Normal("store".as_ref())
        || components[3].as_os_str().is_empty()
        || components[3..]
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("source is outside an exact Nix store object".into());
    }
    Ok(())
}

fn validate_canonical_immutable_store_subpath(path: &Path) -> Result<(), String> {
    validate_immutable_store_subpath(path)?;
    let canonical = fs::canonicalize(path).map_err(|error| error.to_string())?;
    if canonical != path {
        return Err("source is not canonical".into());
    }
    validate_immutable_store_subpath(&canonical)
}

fn validate_directory(path: &Path, expected_owner_uid: u32) -> Result<(), String> {
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

fn sync_dir(path: &Path) -> Result<(), MaterializationBeginError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| MaterializationBeginError::Io(error.to_string()))
}

fn now_unix_ms() -> Result<u64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;
    u64::try_from(elapsed.as_millis()).map_err(|_| "system clock overflow".to_owned())
}

fn random_hex_128() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| error.to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn hex_text(value: &str) -> String {
    value.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex(value: &str) -> Result<String, TrustedCandidateError> {
    if value.len() % 2 != 0 {
        return Err(TrustedCandidateError::Malformed);
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high = nibble(pair[0])?;
        let low = nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }
    String::from_utf8(bytes).map_err(|_| TrustedCandidateError::Malformed)
}

fn nibble(value: u8) -> Result<u8, TrustedCandidateError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(TrustedCandidateError::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_candidate_round_trips_canonically() {
        let candidate = TrustedMaterializationCandidate {
            manifest_id: "manifest:one".into(),
            candidate: SystemCandidateId::from("candidate:one"),
            system_spec: SystemSpecId::from("system:one"),
            immutable_flake_root: PathBuf::from("/nix/store/aaaaaaaa-source"),
            installable_attribute: "packages.x86_64-linux.candidate".into(),
            provenance: vec!["systems-spec:validated".into()],
        };
        let text = canonical_trusted_candidate(&candidate);
        assert_eq!(parse_trusted_candidate(&text), Ok(candidate));
    }

    #[test]
    fn manifest_id_and_attribute_reject_shell_syntax() {
        assert!(validate_manifest_id("manifest;evil").is_err());
        assert!(validate_installable_attribute("x^out").is_err());
        assert!(validate_installable_attribute("x;--impure").is_err());
    }

    #[test]
    fn source_must_be_inside_nix_store() {
        assert!(validate_immutable_store_subpath(Path::new("/tmp/source")).is_err());
        assert!(validate_immutable_store_subpath(Path::new("/nix/store/hash-source/ref")).is_ok());
    }
}
