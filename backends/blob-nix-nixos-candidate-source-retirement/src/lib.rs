#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use blob_nix_nixos_candidate_lease::{CandidateEnqueueLeaseManager, CandidateLeaseError};
use blob_nix_nixos_materialization_lifecycle::{
    LifecycleError, LifecyclePaths, RootMaterializationLifecycleManager,
};

pub const DEFAULT_SOURCE_RETIREMENT_RECEIPT_ROOT: &str =
    "/var/lib/theblob/materialization-lifecycle/source-retirements";
const MAX_RECEIPT_BYTES: u64 = 64 * 1024;

#[derive(Debug)]
pub enum CandidateSourceRetirementError {
    Lease(CandidateLeaseError),
    Lifecycle(LifecycleError),
    MissingSelectionReceipt,
    InvalidSelectionReceipt,
    SourceRootConflict,
    InvalidLayout,
    Io(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateSourceRetirementDisposition {
    Reclaimed,
    AlreadyReclaimed,
}

pub struct RootCandidateSourceRetirement {
    lifecycle: RootMaterializationLifecycleManager,
    lifecycle_paths: LifecyclePaths,
    leases: CandidateEnqueueLeaseManager,
    receipt_root: PathBuf,
    expected_owner_uid: u32,
}

impl RootCandidateSourceRetirement {
    pub fn production_default() -> Self {
        let lifecycle_paths = LifecyclePaths::production_default();
        Self::new(
            lifecycle_paths,
            blob_nix_nixos_candidate_lease::DEFAULT_CANDIDATE_LEASE_ROOT,
            DEFAULT_SOURCE_RETIREMENT_RECEIPT_ROOT,
            0,
        )
    }

    pub fn new(
        lifecycle_paths: LifecyclePaths,
        lease_root: impl Into<PathBuf>,
        receipt_root: impl Into<PathBuf>,
        expected_owner_uid: u32,
    ) -> Self {
        let lifecycle = RootMaterializationLifecycleManager::new(
            lifecycle_paths.clone(),
            expected_owner_uid,
        );
        Self {
            lifecycle,
            lifecycle_paths,
            leases: CandidateEnqueueLeaseManager::new(lease_root, expected_owner_uid),
            receipt_root: receipt_root.into(),
            expected_owner_uid,
        }
    }

    /// Retire selection and source in monotonic phases:
    ///
    /// 1. publish the enqueue retirement barrier and prove no pre-barrier lease remains;
    /// 2. retire candidate selection using the existing lifecycle proof;
    /// 3. prove quiescence again and make the barrier permanently retired;
    /// 4. persist exact source-retirement evidence and release the exact source GC root.
    pub fn retire_candidate_and_source(
        &self,
        manifest_id: &str,
        now_unix_ms: u64,
        retention_ms: u64,
    ) -> Result<CandidateSourceRetirementDisposition, CandidateSourceRetirementError> {
        self.validate_layout()?;
        self.leases
            .begin_retirement(manifest_id)
            .map_err(CandidateSourceRetirementError::Lease)?;
        self.leases
            .require_quiescent(manifest_id)
            .map_err(CandidateSourceRetirementError::Lease)?;

        let selection_receipt = self.selection_receipt_path(manifest_id);
        if !selection_receipt.exists() {
            self.lifecycle
                .retire_candidate(manifest_id, now_unix_ms, retention_ms)
                .map_err(CandidateSourceRetirementError::Lifecycle)?;
        }

        self.leases
            .require_quiescent(manifest_id)
            .map_err(CandidateSourceRetirementError::Lease)?;
        self.leases
            .mark_retired(manifest_id)
            .map_err(CandidateSourceRetirementError::Lease)?;

        let expected_source = self.expected_source_from_selection_receipt(manifest_id)?;
        let source_root = self.source_gcroot_path(manifest_id);
        let receipt = self.source_retirement_receipt_path(manifest_id);

        match fs::symlink_metadata(&source_root) {
            Ok(metadata) => {
                if !metadata.file_type().is_symlink()
                    || fs::read_link(&source_root).map_err(io_error)? != expected_source
                {
                    return Err(CandidateSourceRetirementError::SourceRootConflict);
                }
                self.ensure_source_retirement_receipt(
                    manifest_id,
                    &expected_source,
                    now_unix_ms,
                    "decision:retired-barrier-and-quiescence-proved",
                )?;
                fs::remove_file(&source_root).map_err(io_error)?;
                sync_dir(&self.lifecycle_paths.candidate_source_gcroot_root)?;
                Ok(CandidateSourceRetirementDisposition::Reclaimed)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !receipt.exists() {
                    return Err(CandidateSourceRetirementError::SourceRootConflict);
                }
                self.validate_source_retirement_receipt(manifest_id, &expected_source)?;
                Ok(CandidateSourceRetirementDisposition::AlreadyReclaimed)
            }
            Err(error) => Err(io_error(error)),
        }
    }

    pub fn source_gcroot_path(&self, manifest_id: &str) -> PathBuf {
        self.lifecycle_paths
            .candidate_source_gcroot_root
            .join(format!("manifest-{}-source", hex_text(manifest_id)))
    }

    fn validate_layout(&self) -> Result<(), CandidateSourceRetirementError> {
        validate_directory(
            &self.lifecycle_paths.candidate_source_gcroot_root,
            self.expected_owner_uid,
        )?;
        validate_directory(&self.receipt_root, self.expected_owner_uid)?;
        Ok(())
    }

    fn selection_receipt_path(&self, manifest_id: &str) -> PathBuf {
        self.lifecycle_paths
            .lifecycle_root
            .join("receipts")
            .join(format!("candidate-retirement-{}.receipt", hex_text(manifest_id)))
    }

    fn source_retirement_receipt_path(&self, manifest_id: &str) -> PathBuf {
        self.receipt_root
            .join(format!("candidate-source-{}.receipt", hex_text(manifest_id)))
    }

    fn expected_source_from_selection_receipt(
        &self,
        manifest_id: &str,
    ) -> Result<PathBuf, CandidateSourceRetirementError> {
        let path = self.selection_receipt_path(manifest_id);
        if !path.exists() {
            return Err(CandidateSourceRetirementError::MissingSelectionReceipt);
        }
        let text = read_protected_text(&path, self.expected_owner_uid)?;
        let expected_prefix = format!(
            "theblob-materialization-lifecycle-receipt-v1\nkind={}\nsubject-id={}\n",
            hex_text("candidate-retirement"),
            hex_text(manifest_id)
        );
        if !text.starts_with(&expected_prefix) || !text.ends_with('\n') {
            return Err(CandidateSourceRetirementError::InvalidSelectionReceipt);
        }

        let evidence_count = text
            .lines()
            .find_map(|line| line.strip_prefix("evidence-count="))
            .ok_or(CandidateSourceRetirementError::InvalidSelectionReceipt)?
            .parse::<usize>()
            .map_err(|_| CandidateSourceRetirementError::InvalidSelectionReceipt)?;
        if evidence_count > 4096 {
            return Err(CandidateSourceRetirementError::InvalidSelectionReceipt);
        }

        let lines = text.lines().collect::<Vec<_>>();
        for index in 0..evidence_count {
            let prefix = format!("evidence-{index}=");
            let mut values = lines
                .iter()
                .filter_map(|line| line.strip_prefix(&prefix));
            let value = values
                .next()
                .ok_or(CandidateSourceRetirementError::InvalidSelectionReceipt)?;
            if values.next().is_some() {
                return Err(CandidateSourceRetirementError::InvalidSelectionReceipt);
            }
            let decoded = decode_hex_text(value)?;
            if let Some(source) = decoded.strip_prefix("source-retained:") {
                let source = PathBuf::from(source);
                if !is_exact_nix_store_path(&source) {
                    return Err(CandidateSourceRetirementError::InvalidSelectionReceipt);
                }
                return Ok(source);
            }
        }
        Err(CandidateSourceRetirementError::InvalidSelectionReceipt)
    }

    fn ensure_source_retirement_receipt(
        &self,
        manifest_id: &str,
        source: &Path,
        now_unix_ms: u64,
        decision: &str,
    ) -> Result<(), CandidateSourceRetirementError> {
        let path = self.source_retirement_receipt_path(manifest_id);
        if path.exists() {
            return self.validate_source_retirement_receipt(manifest_id, source);
        }
        let text = canonical_source_retirement_receipt(
            manifest_id,
            source,
            now_unix_ms,
            decision,
        );
        create_protected_file(&path, &text)?;
        sync_dir(&self.receipt_root)?;
        self.validate_source_retirement_receipt(manifest_id, source)
    }

    fn validate_source_retirement_receipt(
        &self,
        manifest_id: &str,
        source: &Path,
    ) -> Result<(), CandidateSourceRetirementError> {
        let text = read_protected_text(
            &self.source_retirement_receipt_path(manifest_id),
            self.expected_owner_uid,
        )?;
        let mut lines = text.lines();
        if lines.next() != Some("theblob-candidate-source-retirement-v1") {
            return Err(CandidateSourceRetirementError::InvalidSelectionReceipt);
        }
        if lines.next() != Some(format!("manifest-id={}", hex_text(manifest_id)).as_str()) {
            return Err(CandidateSourceRetirementError::InvalidSelectionReceipt);
        }
        if lines.next() != Some(format!("source={}", hex_text(&source.display().to_string())).as_str()) {
            return Err(CandidateSourceRetirementError::InvalidSelectionReceipt);
        }
        let occurred = lines
            .next()
            .and_then(|line| line.strip_prefix("occurred-at-unix-ms="))
            .ok_or(CandidateSourceRetirementError::InvalidSelectionReceipt)?
            .parse::<u64>()
            .map_err(|_| CandidateSourceRetirementError::InvalidSelectionReceipt)?;
        let decision_hex = lines
            .next()
            .and_then(|line| line.strip_prefix("decision="))
            .ok_or(CandidateSourceRetirementError::InvalidSelectionReceipt)?;
        let decision = decode_hex_text(decision_hex)?;
        if lines.next().is_some() || !text.ends_with('\n') {
            return Err(CandidateSourceRetirementError::InvalidSelectionReceipt);
        }
        if canonical_source_retirement_receipt(manifest_id, source, occurred, &decision) != text {
            return Err(CandidateSourceRetirementError::InvalidSelectionReceipt);
        }
        Ok(())
    }
}

fn canonical_source_retirement_receipt(
    manifest_id: &str,
    source: &Path,
    now_unix_ms: u64,
    decision: &str,
) -> String {
    format!(
        "theblob-candidate-source-retirement-v1\nmanifest-id={}\nsource={}\noccurred-at-unix-ms={}\ndecision={}\n",
        hex_text(manifest_id),
        hex_text(&source.display().to_string()),
        now_unix_ms,
        hex_text(decision),
    )
}

fn validate_directory(
    path: &Path,
    expected_owner_uid: u32,
) -> Result<(), CandidateSourceRetirementError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != expected_owner_uid
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(CandidateSourceRetirementError::InvalidLayout);
    }
    Ok(())
}

fn read_protected_text(
    path: &Path,
    expected_owner_uid: u32,
) -> Result<String, CandidateSourceRetirementError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != expected_owner_uid
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.len() > MAX_RECEIPT_BYTES
    {
        return Err(CandidateSourceRetirementError::InvalidSelectionReceipt);
    }
    let mut text = String::new();
    File::open(path)
        .and_then(|mut file| file.read_to_string(&mut text))
        .map_err(io_error)?;
    if text.len() as u64 > MAX_RECEIPT_BYTES {
        return Err(CandidateSourceRetirementError::InvalidSelectionReceipt);
    }
    Ok(text)
}

fn create_protected_file(
    path: &Path,
    text: &str,
) -> Result<(), CandidateSourceRetirementError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(io_error)?;
    file.write_all(text.as_bytes()).map_err(io_error)?;
    file.sync_all().map_err(io_error)
}

fn sync_dir(path: &Path) -> Result<(), CandidateSourceRetirementError> {
    File::open(path).and_then(|file| file.sync_all()).map_err(io_error)
}

fn io_error(error: std::io::Error) -> CandidateSourceRetirementError {
    CandidateSourceRetirementError::Io(error.to_string())
}

fn hex_text(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn decode_hex_text(value: &str) -> Result<String, CandidateSourceRetirementError> {
    if value.len() % 2 != 0 {
        return Err(CandidateSourceRetirementError::InvalidSelectionReceipt);
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let text = std::str::from_utf8(pair)
            .map_err(|_| CandidateSourceRetirementError::InvalidSelectionReceipt)?;
        let byte = u8::from_str_radix(text, 16)
            .map_err(|_| CandidateSourceRetirementError::InvalidSelectionReceipt)?;
        bytes.push(byte);
    }
    String::from_utf8(bytes).map_err(|_| CandidateSourceRetirementError::InvalidSelectionReceipt)
}

fn is_exact_nix_store_path(path: &Path) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    if !text.starts_with("/nix/store/") || text.ends_with('/') {
        return false;
    }
    Path::new(text).components().count() == 4
}
