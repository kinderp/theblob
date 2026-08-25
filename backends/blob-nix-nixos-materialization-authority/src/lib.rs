#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};

use blob_core::{
    NodeId, SystemCandidateId, SystemOperationId, SystemSpecId,
};
use blob_nix_nixos_request_publisher::{
    canonical_materialization_admission, FileMaterializationAdmissionStore,
    MaterializationAdmission,
};

pub const DEFAULT_MATERIALIZATION_INTENT_ROOT: &str =
    "/var/lib/theblob/materialization-intents";
pub const DEFAULT_MATERIALIZATION_ADMISSION_ROOT: &str =
    "/var/lib/theblob/materialization-admissions";
pub const MAX_MATERIALIZATION_INTENT_BYTES: u64 = 32 * 1024;

const PENDING: &str = "pending";
const COMPLETED: &str = "completed";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterializationIntentSpec {
    pub node: NodeId,
    pub candidate: SystemCandidateId,
    pub system_spec: SystemSpecId,
    pub materialization_operation: SystemOperationId,
    pub immutable_flake_root: PathBuf,
    pub installable_attribute: String,
    pub created_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterializationIntent {
    pub node: NodeId,
    pub candidate: SystemCandidateId,
    pub system_spec: SystemSpecId,
    pub materialization_operation: SystemOperationId,
    pub immutable_flake_root: PathBuf,
    pub installable_attribute: String,
    pub derivation_path: PathBuf,
    pub expected_output: PathBuf,
    pub created_at_unix_ms: u64,
}

impl MaterializationIntent {
    pub fn build_target(&self) -> String {
        format!("{}^out", self.derivation_path.display())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedMaterializationDerivation {
    pub derivation_path: PathBuf,
    pub expected_output: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedMaterializationOutput {
    pub deriver: PathBuf,
    pub nar_hash: String,
}

pub trait NixMaterializationInspector {
    fn resolve_exact_derivation(
        &self,
        immutable_flake_root: &Path,
        installable_attribute: &str,
    ) -> Result<ResolvedMaterializationDerivation, String>;

    fn verify_realized_output(
        &self,
        derivation_path: &Path,
        expected_output: &Path,
    ) -> Result<VerifiedMaterializationOutput, String>;
}

pub struct StdNixMaterializationInspector {
    nix_program: PathBuf,
    nix_store_program: PathBuf,
}

impl StdNixMaterializationInspector {
    pub fn new(nix_program: impl Into<PathBuf>, nix_store_program: impl Into<PathBuf>) -> Self {
        Self {
            nix_program: nix_program.into(),
            nix_store_program: nix_store_program.into(),
        }
    }

    fn run(program: &Path, args: &[String]) -> Result<Output, String> {
        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .env_clear()
            .env("HOME", "/root")
            .env("USER", "root")
            .env("LOGNAME", "root")
            .env("LANG", "C")
            .output()
            .map_err(|error| error.to_string())
    }

    fn succeed(program: &Path, args: &[String]) -> Result<String, String> {
        let output = Self::run(program, args)?;
        if !output.status.success() {
            return Err(format!(
                "{} {:?} failed with {:?}: {}",
                program.display(),
                args,
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl NixMaterializationInspector for StdNixMaterializationInspector {
    fn resolve_exact_derivation(
        &self,
        immutable_flake_root: &Path,
        installable_attribute: &str,
    ) -> Result<ResolvedMaterializationDerivation, String> {
        validate_canonical_immutable_store_subpath(immutable_flake_root)?;
        validate_installable_attribute(installable_attribute)?;
        let installable = format!(
            "{}#{}",
            immutable_flake_root.display(),
            installable_attribute
        );
        let drv_stdout = Self::succeed(
            &self.nix_program,
            &[
                "path-info".into(),
                "--derivation".into(),
                "--no-write-lock-file".into(),
                installable,
            ],
        )?;
        let derivation_path = one_store_line(&drv_stdout, true)?;

        let outputs_stdout = Self::succeed(
            &self.nix_store_program,
            &[
                "--query".into(),
                "--outputs".into(),
                derivation_path.display().to_string(),
            ],
        )?;
        let expected_output = one_store_line(&outputs_stdout, false)?;
        Ok(ResolvedMaterializationDerivation {
            derivation_path,
            expected_output,
        })
    }

    fn verify_realized_output(
        &self,
        derivation_path: &Path,
        expected_output: &Path,
    ) -> Result<VerifiedMaterializationOutput, String> {
        validate_derivation_path(derivation_path)?;
        validate_output_path(expected_output)?;

        // `path-info` does not realize a missing output. Completion therefore
        // cannot silently turn a root verification call into the materializer.
        Self::succeed(
            &self.nix_program,
            &["path-info".into(), expected_output.display().to_string()],
        )?;
        Self::succeed(
            &self.nix_program,
            &[
                "store".into(),
                "verify".into(),
                "--no-trust".into(),
                expected_output.display().to_string(),
            ],
        )?;

        let deriver_stdout = Self::succeed(
            &self.nix_store_program,
            &[
                "--query".into(),
                "--deriver".into(),
                expected_output.display().to_string(),
            ],
        )?;
        let observed_deriver = PathBuf::from(deriver_stdout.trim());
        if observed_deriver != derivation_path {
            return Err(format!(
                "output deriver mismatch: expected {}, observed {}",
                derivation_path.display(),
                observed_deriver.display()
            ));
        }

        let nar_hash = Self::succeed(
            &self.nix_store_program,
            &[
                "--query".into(),
                "--hash".into(),
                expected_output.display().to_string(),
            ],
        )?
        .trim()
        .to_owned();
        if nar_hash.is_empty() {
            return Err("nix store returned an empty output hash".into());
        }

        Ok(VerifiedMaterializationOutput {
            deriver: observed_deriver,
            nar_hash,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MaterializationAuthorityError {
    InvalidIntentRoot,
    InvalidAdmissionRoot,
    InvalidImmutableFlakeRoot,
    InvalidInstallableAttribute,
    InvalidDerivation,
    InvalidExpectedOutput,
    IntentAlreadyExists(SystemOperationId),
    IntentMissing(SystemOperationId),
    InvalidIntentFile,
    NonCanonicalIntent,
    IntentMismatch,
    AdmissionConflict,
    Inspector(String),
    Io(String),
}

pub struct RootMaterializationAdmissionAuthority {
    intent_root: PathBuf,
    admission_root: PathBuf,
    expected_owner_uid: u32,
}

impl RootMaterializationAdmissionAuthority {
    pub fn production_default() -> Self {
        Self::new(
            DEFAULT_MATERIALIZATION_INTENT_ROOT,
            DEFAULT_MATERIALIZATION_ADMISSION_ROOT,
            0,
        )
    }

    pub fn new(
        intent_root: impl Into<PathBuf>,
        admission_root: impl Into<PathBuf>,
        expected_owner_uid: u32,
    ) -> Self {
        Self {
            intent_root: intent_root.into(),
            admission_root: admission_root.into(),
            expected_owner_uid,
        }
    }

    pub fn begin<I: NixMaterializationInspector>(
        &self,
        spec: &MaterializationIntentSpec,
        inspector: &I,
    ) -> Result<MaterializationIntent, MaterializationAuthorityError> {
        self.validate_layout()?;
        validate_canonical_immutable_store_subpath(&spec.immutable_flake_root)
            .map_err(|_| MaterializationAuthorityError::InvalidImmutableFlakeRoot)?;
        validate_installable_attribute(&spec.installable_attribute)
            .map_err(|_| MaterializationAuthorityError::InvalidInstallableAttribute)?;

        for state in [PENDING, COMPLETED] {
            if path_exists(&self.intent_path(state, &spec.materialization_operation))? {
                return Err(MaterializationAuthorityError::IntentAlreadyExists(
                    spec.materialization_operation.clone(),
                ));
            }
        }
        if path_exists(&self.admission_path(&spec.materialization_operation))? {
            return Err(MaterializationAuthorityError::AdmissionConflict);
        }

        let resolved = inspector
            .resolve_exact_derivation(&spec.immutable_flake_root, &spec.installable_attribute)
            .map_err(MaterializationAuthorityError::Inspector)?;
        validate_derivation_path(&resolved.derivation_path)
            .map_err(|_| MaterializationAuthorityError::InvalidDerivation)?;
        validate_output_path(&resolved.expected_output)
            .map_err(|_| MaterializationAuthorityError::InvalidExpectedOutput)?;

        let intent = MaterializationIntent {
            node: spec.node.clone(),
            candidate: spec.candidate.clone(),
            system_spec: spec.system_spec.clone(),
            materialization_operation: spec.materialization_operation.clone(),
            immutable_flake_root: spec.immutable_flake_root.clone(),
            installable_attribute: spec.installable_attribute.clone(),
            derivation_path: resolved.derivation_path,
            expected_output: resolved.expected_output,
            created_at_unix_ms: spec.created_at_unix_ms,
        };
        let path = self.intent_path(PENDING, &intent.materialization_operation);
        create_root_file(&path, &canonical_intent(&intent))?;
        sync_dir(&self.intent_root.join(PENDING))?;
        Ok(intent)
    }

    /// Complete without accepting an output path from the caller. Root reloads
    /// its durable intent and verifies only the output path it predicted before
    /// the non-privileged build was allowed to start.
    pub fn complete<I: NixMaterializationInspector>(
        &self,
        operation: &SystemOperationId,
        admitted_at_unix_ms: u64,
        inspector: &I,
    ) -> Result<MaterializationAdmission, MaterializationAuthorityError> {
        self.validate_layout()?;
        let intent = self.load_intent(PENDING, operation)?;
        let verified = inspector
            .verify_realized_output(&intent.derivation_path, &intent.expected_output)
            .map_err(MaterializationAuthorityError::Inspector)?;
        if verified.deriver != intent.derivation_path {
            return Err(MaterializationAuthorityError::IntentMismatch);
        }

        let admission = MaterializationAdmission {
            node: intent.node.clone(),
            candidate: intent.candidate.clone(),
            system_spec: intent.system_spec.clone(),
            materialization_operation: intent.materialization_operation.clone(),
            system_closure: intent.expected_output.display().to_string(),
            admitted_at_unix_ms,
            provenance: vec![
                format!("immutable-flake-root:{}", intent.immutable_flake_root.display()),
                format!("installable-attribute:{}", intent.installable_attribute),
                format!("derivation:{}", intent.derivation_path.display()),
                format!("expected-output:{}", intent.expected_output.display()),
                format!("verified-deriver:{}", verified.deriver.display()),
                format!("verified-nar-hash:{}", verified.nar_hash),
                "verification:nix-path-info+store-verify+deriver".into(),
            ],
        };
        self.persist_or_reconcile_admission(&admission)?;

        let pending = self.intent_path(PENDING, operation);
        let completed = self.intent_path(COMPLETED, operation);
        if path_exists(&pending)? {
            if path_exists(&completed)? {
                return Err(MaterializationAuthorityError::IntentMismatch);
            }
            fs::rename(&pending, &completed)
                .map_err(|error| MaterializationAuthorityError::Io(error.to_string()))?;
            sync_dir(&self.intent_root.join(PENDING))?;
            sync_dir(&self.intent_root.join(COMPLETED))?;
        }
        Ok(admission)
    }

    pub fn load_pending(
        &self,
        operation: &SystemOperationId,
    ) -> Result<MaterializationIntent, MaterializationAuthorityError> {
        self.validate_layout()?;
        self.load_intent(PENDING, operation)
    }

    fn persist_or_reconcile_admission(
        &self,
        admission: &MaterializationAdmission,
    ) -> Result<(), MaterializationAuthorityError> {
        let path = self.admission_path(&admission.materialization_operation);
        if path_exists(&path)? {
            let store = FileMaterializationAdmissionStore::new(
                &self.admission_root,
                self.expected_owner_uid,
            );
            let existing = store
                .load(&admission.materialization_operation)
                .map_err(|_| MaterializationAuthorityError::AdmissionConflict)?;
            if existing != *admission {
                return Err(MaterializationAuthorityError::AdmissionConflict);
            }
            return Ok(());
        }
        create_root_file(&path, &canonical_materialization_admission(admission))?;
        sync_dir(&self.admission_root)?;
        Ok(())
    }

    fn load_intent(
        &self,
        state: &str,
        operation: &SystemOperationId,
    ) -> Result<MaterializationIntent, MaterializationAuthorityError> {
        let path = self.intent_path(state, operation);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                MaterializationAuthorityError::IntentMissing(operation.clone())
            } else {
                MaterializationAuthorityError::Io(error.to_string())
            }
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != self.expected_owner_uid
            || metadata.permissions().mode() & 0o777 != 0o600
            || metadata.len() > MAX_MATERIALIZATION_INTENT_BYTES
        {
            return Err(MaterializationAuthorityError::InvalidIntentFile);
        }
        let mut text = String::new();
        File::open(&path)
            .and_then(|mut file| file.read_to_string(&mut text))
            .map_err(|error| MaterializationAuthorityError::Io(error.to_string()))?;
        let intent = parse_intent(&text)?;
        if intent.materialization_operation != *operation {
            return Err(MaterializationAuthorityError::IntentMismatch);
        }
        if canonical_intent(&intent) != text {
            return Err(MaterializationAuthorityError::NonCanonicalIntent);
        }
        Ok(intent)
    }

    fn validate_layout(&self) -> Result<(), MaterializationAuthorityError> {
        validate_directory(&self.intent_root, self.expected_owner_uid)
            .map_err(|_| MaterializationAuthorityError::InvalidIntentRoot)?;
        for state in [PENDING, COMPLETED] {
            validate_directory(&self.intent_root.join(state), self.expected_owner_uid)
                .map_err(|_| MaterializationAuthorityError::InvalidIntentRoot)?;
        }
        validate_directory(&self.admission_root, self.expected_owner_uid)
            .map_err(|_| MaterializationAuthorityError::InvalidAdmissionRoot)?;
        Ok(())
    }

    fn intent_path(&self, state: &str, operation: &SystemOperationId) -> PathBuf {
        self.intent_root.join(state).join(format!(
            "operation-{}.intent",
            hex_text(operation.as_str())
        ))
    }

    fn admission_path(&self, operation: &SystemOperationId) -> PathBuf {
        self.admission_root.join(format!(
            "operation-{}.admission",
            hex_text(operation.as_str())
        ))
    }
}

pub fn canonical_intent(intent: &MaterializationIntent) -> String {
    [
        "theblob-materialization-intent-v1".to_owned(),
        format!(
            "materialization-operation={}",
            hex_text(intent.materialization_operation.as_str())
        ),
        format!("node={}", hex_text(intent.node.as_str())),
        format!("candidate={}", hex_text(intent.candidate.as_str())),
        format!("system-spec={}", hex_text(intent.system_spec.as_str())),
        format!(
            "immutable-flake-root={}",
            hex_text(&intent.immutable_flake_root.display().to_string())
        ),
        format!(
            "installable-attribute={}",
            hex_text(&intent.installable_attribute)
        ),
        format!(
            "derivation-path={}",
            hex_text(&intent.derivation_path.display().to_string())
        ),
        format!(
            "expected-output={}",
            hex_text(&intent.expected_output.display().to_string())
        ),
        format!("created-at-unix-ms={}", intent.created_at_unix_ms),
        String::new(),
    ]
    .join("\n")
}

pub fn parse_intent(text: &str) -> Result<MaterializationIntent, MaterializationAuthorityError> {
    if text.len() as u64 > MAX_MATERIALIZATION_INTENT_BYTES {
        return Err(MaterializationAuthorityError::InvalidIntentFile);
    }
    let mut cursor = Cursor::new(text);
    cursor.literal("theblob-materialization-intent-v1")?;
    let materialization_operation =
        SystemOperationId::from(cursor.hex_field("materialization-operation")?);
    let node = NodeId::from(cursor.hex_field("node")?);
    let candidate = SystemCandidateId::from(cursor.hex_field("candidate")?);
    let system_spec = SystemSpecId::from(cursor.hex_field("system-spec")?);
    let immutable_flake_root = PathBuf::from(cursor.hex_field("immutable-flake-root")?);
    let installable_attribute = cursor.hex_field("installable-attribute")?;
    let derivation_path = PathBuf::from(cursor.hex_field("derivation-path")?);
    let expected_output = PathBuf::from(cursor.hex_field("expected-output")?);
    let created_at_unix_ms = cursor.u64_field("created-at-unix-ms")?;
    cursor.finish()?;
    validate_immutable_store_subpath(&immutable_flake_root)
        .map_err(|_| MaterializationAuthorityError::InvalidImmutableFlakeRoot)?;
    validate_installable_attribute(&installable_attribute)
        .map_err(|_| MaterializationAuthorityError::InvalidInstallableAttribute)?;
    validate_derivation_path(&derivation_path)
        .map_err(|_| MaterializationAuthorityError::InvalidDerivation)?;
    validate_output_path(&expected_output)
        .map_err(|_| MaterializationAuthorityError::InvalidExpectedOutput)?;
    Ok(MaterializationIntent {
        node,
        candidate,
        system_spec,
        materialization_operation,
        immutable_flake_root,
        installable_attribute,
        derivation_path,
        expected_output,
        created_at_unix_ms,
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

    fn next(&mut self) -> Result<&'a str, MaterializationAuthorityError> {
        let line = self.lines.get(self.position).copied().ok_or_else(|| {
            MaterializationAuthorityError::InvalidIntentFile
        })?;
        self.position += 1;
        Ok(line)
    }

    fn literal(&mut self, expected: &str) -> Result<(), MaterializationAuthorityError> {
        if self.next()? != expected {
            return Err(MaterializationAuthorityError::InvalidIntentFile);
        }
        Ok(())
    }

    fn field(&mut self, key: &str) -> Result<&'a str, MaterializationAuthorityError> {
        let line = self.next()?;
        line.strip_prefix(&format!("{key}="))
            .ok_or(MaterializationAuthorityError::InvalidIntentFile)
    }

    fn hex_field(&mut self, key: &str) -> Result<String, MaterializationAuthorityError> {
        decode_hex(self.field(key)?)
    }

    fn u64_field(&mut self, key: &str) -> Result<u64, MaterializationAuthorityError> {
        self.field(key)?
            .parse::<u64>()
            .map_err(|_| MaterializationAuthorityError::InvalidIntentFile)
    }

    fn finish(&mut self) -> Result<(), MaterializationAuthorityError> {
        if !self.next()?.is_empty() || self.position != self.lines.len() {
            return Err(MaterializationAuthorityError::InvalidIntentFile);
        }
        Ok(())
    }
}

fn one_store_line(text: &str, derivation: bool) -> Result<PathBuf, String> {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() != 1 {
        return Err(format!("expected exactly one Nix store path, observed {lines:?}"));
    }
    let path = PathBuf::from(lines[0]);
    if derivation {
        validate_derivation_path(&path)?;
    } else {
        validate_output_path(&path)?;
    }
    Ok(path)
}

fn validate_immutable_store_subpath(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("flake root is not absolute".into());
    }
    let components = path.components().collect::<Vec<_>>();
    if components.len() < 4
        || components[0] != Component::RootDir
        || components[1] != Component::Normal("nix".as_ref())
        || components[2] != Component::Normal("store".as_ref())
    {
        return Err("flake root is outside /nix/store".into());
    }
    if components[3].as_os_str().is_empty()
        || components[3..]
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("flake root has unsafe path components".into());
    }
    Ok(())
}

fn validate_canonical_immutable_store_subpath(path: &Path) -> Result<(), String> {
    validate_immutable_store_subpath(path)?;
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("failed to canonicalize immutable flake root: {error}"))?;
    if canonical != path {
        return Err(format!(
            "immutable flake root is not canonical: {} -> {}",
            path.display(),
            canonical.display()
        ));
    }
    validate_immutable_store_subpath(&canonical)
}

fn validate_installable_attribute(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 512 {
        return Err("invalid installable attribute length".into());
    }
    if !value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')
    }) {
        return Err("invalid installable attribute".into());
    }
    Ok(())
}

fn validate_derivation_path(path: &Path) -> Result<(), String> {
    validate_exact_store_path(path)?;
    if path.extension().and_then(|value| value.to_str()) != Some("drv") {
        return Err("path is not a .drv store derivation".into());
    }
    Ok(())
}

fn validate_output_path(path: &Path) -> Result<(), String> {
    validate_exact_store_path(path)?;
    if path.extension().and_then(|value| value.to_str()) == Some("drv") {
        return Err("expected output cannot be a derivation path".into());
    }
    Ok(())
}

fn validate_exact_store_path(path: &Path) -> Result<(), String> {
    let components = path.components().collect::<Vec<_>>();
    if matches!(
        components.as_slice(),
        [
            Component::RootDir,
            Component::Normal(nix),
            Component::Normal(store),
            Component::Normal(name)
        ] if *nix == "nix" && *store == "store" && !name.is_empty()
    ) {
        Ok(())
    } else {
        Err("invalid exact Nix store path".into())
    }
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

fn create_root_file(path: &Path, text: &str) -> Result<(), MaterializationAuthorityError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| MaterializationAuthorityError::Io(error.to_string()))?;
    file.write_all(text.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| MaterializationAuthorityError::Io(error.to_string()))?;
    Ok(())
}

fn sync_dir(path: &Path) -> Result<(), MaterializationAuthorityError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| MaterializationAuthorityError::Io(error.to_string()))
}

fn path_exists(path: &Path) -> Result<bool, MaterializationAuthorityError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(MaterializationAuthorityError::Io(error.to_string())),
    }
}

fn hex_text(value: &str) -> String {
    value.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex(value: &str) -> Result<String, MaterializationAuthorityError> {
    if value.len() % 2 != 0 {
        return Err(MaterializationAuthorityError::InvalidIntentFile);
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high = nibble(pair[0])?;
        let low = nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }
    String::from_utf8(bytes).map_err(|_| MaterializationAuthorityError::InvalidIntentFile)
}

fn nibble(value: u8) -> Result<u8, MaterializationAuthorityError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(MaterializationAuthorityError::InvalidIntentFile),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intent_round_trips_exactly() {
        let intent = MaterializationIntent {
            node: NodeId::from("node:one"),
            candidate: SystemCandidateId::from("candidate:one"),
            system_spec: SystemSpecId::from("system:one"),
            materialization_operation: SystemOperationId::from("op:one"),
            immutable_flake_root: PathBuf::from("/nix/store/aaaaaaaa-source/ref"),
            installable_attribute: "packages.x86_64-linux.default".into(),
            derivation_path: PathBuf::from("/nix/store/bbbbbbbb-output.drv"),
            expected_output: PathBuf::from("/nix/store/cccccccc-output"),
            created_at_unix_ms: 42,
        };
        let text = canonical_intent(&intent);
        assert_eq!(parse_intent(&text), Ok(intent));
    }

    #[test]
    fn unsafe_installable_attribute_is_rejected() {
        assert!(validate_installable_attribute("x;--impure").is_err());
        assert!(validate_installable_attribute("x^out").is_err());
    }

    #[test]
    fn source_must_be_inside_an_immutable_store_object() {
        assert!(validate_immutable_store_subpath(Path::new("/tmp/source")).is_err());
        assert!(validate_immutable_store_subpath(Path::new("/nix/store/hash-source/ref")).is_ok());
    }
}
