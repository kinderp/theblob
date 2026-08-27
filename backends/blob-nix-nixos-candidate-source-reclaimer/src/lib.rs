#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use blob_nix_nixos_candidate_lease::{
    CandidateEnqueueLeaseError, FileCandidateEnqueueLeaseStore,
    DEFAULT_CANDIDATE_ENQUEUE_LEASE_ROOT,
};
use blob_nix_nixos_candidate_producer::{
    DEFAULT_CANDIDATE_RECEIPT_ROOT, DEFAULT_CANDIDATE_SOURCE_GCROOT_ROOT,
};
use blob_nix_nixos_materialization_begin::DEFAULT_TRUSTED_CANDIDATE_ROOT;
use blob_nix_nixos_materialization_begin_queue::{
    canonical_begin_job, parse_begin_job, DEFAULT_BEGIN_JOB_ROOT, MAX_BEGIN_JOB_BYTES,
};
use blob_nix_nixos_materialization_lifecycle::{
    canonical_lifecycle_receipt, DEFAULT_MATERIALIZATION_LIFECYCLE_ROOT,
    MAX_LIFECYCLE_RECEIPTS, MAX_LIFECYCLE_RECORD_BYTES,
};

const QUEUED: &str = "queued";
const RUNNING: &str = "running";
const COMPLETED: &str = "completed";
const FAILED: &str = "failed";
const RECEIPTS: &str = "receipts";
const CANDIDATE_RETIREMENT: &str = "candidate-retirement";
const SOURCE_RETIREMENT: &str = "candidate-source-retirement";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateSourceReclaimDisposition {
    Reclaimed,
    AlreadyReclaimed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateSourceReclaimError {
    InvalidLayout(String),
    InvalidManifestId,
    SelectionStillPresent,
    ProducerReceiptStillPresent,
    MissingCandidateRetirementReceipt,
    InvalidLifecycleReceipt(String),
    ActiveEnqueueLeases(usize),
    ActiveBeginJobs(usize),
    MissingSourceRoot,
    SourceRootConflict,
    SourceMissing,
    ReceiptCapacityExceeded,
    Lease(String),
    Io(String),
}

#[derive(Clone, Debug)]
pub struct CandidateSourceReclaimerPaths {
    pub candidate_root: PathBuf,
    pub producer_receipt_root: PathBuf,
    pub begin_job_root: PathBuf,
    pub enqueue_lease_root: PathBuf,
    pub lifecycle_root: PathBuf,
    pub source_gcroot_root: PathBuf,
}

impl CandidateSourceReclaimerPaths {
    pub fn production_default() -> Self {
        Self {
            candidate_root: PathBuf::from(DEFAULT_TRUSTED_CANDIDATE_ROOT),
            producer_receipt_root: PathBuf::from(DEFAULT_CANDIDATE_RECEIPT_ROOT),
            begin_job_root: PathBuf::from(DEFAULT_BEGIN_JOB_ROOT),
            enqueue_lease_root: PathBuf::from(DEFAULT_CANDIDATE_ENQUEUE_LEASE_ROOT),
            lifecycle_root: PathBuf::from(DEFAULT_MATERIALIZATION_LIFECYCLE_ROOT),
            source_gcroot_root: PathBuf::from(DEFAULT_CANDIDATE_SOURCE_GCROOT_ROOT),
        }
    }
}

pub struct RootCandidateSourceReclaimer {
    paths: CandidateSourceReclaimerPaths,
    leases: FileCandidateEnqueueLeaseStore,
    expected_owner_uid: u32,
}

impl RootCandidateSourceReclaimer {
    pub fn production_default() -> Self {
        Self::new(CandidateSourceReclaimerPaths::production_default(), 0)
    }

    pub fn new(paths: CandidateSourceReclaimerPaths, expected_owner_uid: u32) -> Self {
        let leases = FileCandidateEnqueueLeaseStore::new(
            paths.enqueue_lease_root.clone(),
            expected_owner_uid,
        );
        Self {
            paths,
            leases,
            expected_owner_uid,
        }
    }

    /// Reclaim one candidate source only after selection retirement, exact lease
    /// drain and exact job drain are all visible at the same root-owned boundary.
    ///
    /// Safety argument for the enqueue race:
    /// - an enqueue that loaded the manifest before retirement must still own its
    ///   durable lease until its queued job has been fsynced;
    /// - therefore after observing zero leases, any successful old enqueue is
    ///   already represented by a job and the subsequent job scan sees it;
    /// - an enqueue started after manifest removal may create a lease, but cannot
    ///   load the retired manifest and therefore never depends on this source.
    pub fn reclaim(
        &self,
        manifest_id: &str,
        now_unix_ms: u64,
    ) -> Result<CandidateSourceReclaimDisposition, CandidateSourceReclaimError> {
        validate_manifest_id(manifest_id)?;
        self.validate_layout()?;

        if path_exists(&self.candidate_manifest_path(manifest_id))? {
            return Err(CandidateSourceReclaimError::SelectionStillPresent);
        }
        if path_exists(&self.producer_receipt_path(manifest_id))? {
            return Err(CandidateSourceReclaimError::ProducerReceiptStillPresent);
        }

        let retirement = self.load_required_receipt(CANDIDATE_RETIREMENT, manifest_id)?;
        let source = retirement_source(&retirement)?;
        validate_exact_store_path(&source)?;

        // Destructive reclamation must never create a missing lease directory and
        // then infer that it is empty. Queue startup is responsible for ensuring
        // the directory; lifecycle only accepts an already-valid protected root.
        self.leases
            .validate_existing_layout()
            .map_err(lease_error)?;
        let active_leases = self.leases.for_manifest(manifest_id).map_err(lease_error)?;
        if !active_leases.is_empty() {
            return Err(CandidateSourceReclaimError::ActiveEnqueueLeases(
                active_leases.len(),
            ));
        }

        let active_jobs = self.jobs_for_manifest(manifest_id)?;
        if active_jobs != 0 {
            return Err(CandidateSourceReclaimError::ActiveBeginJobs(active_jobs));
        }

        let source_root = self.source_gcroot_path(manifest_id);
        if let Some(existing) = self.load_optional_receipt(SOURCE_RETIREMENT, manifest_id)? {
            require_source_retirement_identity(&existing, &source)?;
            if !path_exists(&source_root)? {
                return Ok(CandidateSourceReclaimDisposition::AlreadyReclaimed);
            }
            require_symlink_target(&source_root, &source)?;
            fs::remove_file(&source_root).map_err(io_error)?;
            sync_dir(&self.paths.source_gcroot_root)?;
            return Ok(CandidateSourceReclaimDisposition::Reclaimed);
        }

        require_symlink_target(&source_root, &source)?;
        if !source.exists() {
            return Err(CandidateSourceReclaimError::SourceMissing);
        }

        self.create_source_retirement_receipt(manifest_id, &source, now_unix_ms)?;
        fs::remove_file(&source_root).map_err(io_error)?;
        sync_dir(&self.paths.source_gcroot_root)?;
        Ok(CandidateSourceReclaimDisposition::Reclaimed)
    }

    pub fn source_gcroot_path(&self, manifest_id: &str) -> PathBuf {
        self.paths.source_gcroot_root.join(format!(
            "manifest-{}-source",
            hex_text(manifest_id)
        ))
    }

    fn validate_layout(&self) -> Result<(), CandidateSourceReclaimError> {
        for path in [
            &self.paths.candidate_root,
            &self.paths.producer_receipt_root,
            &self.paths.begin_job_root,
            &self.paths.lifecycle_root,
            &self.receipt_root(),
            &self.paths.source_gcroot_root,
        ] {
            validate_directory(path, self.expected_owner_uid)?;
        }
        for state in [QUEUED, RUNNING, COMPLETED, FAILED] {
            validate_directory(
                &self.paths.begin_job_root.join(state),
                self.expected_owner_uid,
            )?;
        }
        Ok(())
    }

    fn jobs_for_manifest(&self, manifest_id: &str) -> Result<usize, CandidateSourceReclaimError> {
        let mut matching = 0usize;
        for state in [QUEUED, RUNNING, COMPLETED, FAILED] {
            let directory = self.paths.begin_job_root.join(state);
            let mut paths = fs::read_dir(&directory)
                .map_err(io_error)?
                .map(|entry| entry.map(|value| value.path()).map_err(io_error))
                .collect::<Result<Vec<_>, _>>()?;
            paths.sort();
            for path in paths {
                if path.extension().and_then(|value| value.to_str()) != Some("job") {
                    return Err(CandidateSourceReclaimError::InvalidLayout(
                        path.display().to_string(),
                    ));
                }
                let text = read_protected_text(
                    &path,
                    self.expected_owner_uid,
                    MAX_BEGIN_JOB_BYTES,
                )?;
                let job = parse_begin_job(&text).map_err(|error| {
                    CandidateSourceReclaimError::InvalidLayout(format!(
                        "invalid begin job {}: {error:?}",
                        path.display()
                    ))
                })?;
                if canonical_begin_job(&job) != text
                    || path
                        != directory.join(format!(
                            "request-{}.job",
                            hex_text(&job.request_id)
                        ))
                {
                    return Err(CandidateSourceReclaimError::InvalidLayout(
                        path.display().to_string(),
                    ));
                }
                if job.manifest_id == manifest_id {
                    matching += 1;
                }
            }
        }
        Ok(matching)
    }

    fn load_required_receipt(
        &self,
        kind: &str,
        subject_id: &str,
    ) -> Result<LifecycleReceipt, CandidateSourceReclaimError> {
        self.load_optional_receipt(kind, subject_id)?
            .ok_or(CandidateSourceReclaimError::MissingCandidateRetirementReceipt)
    }

    fn load_optional_receipt(
        &self,
        kind: &str,
        subject_id: &str,
    ) -> Result<Option<LifecycleReceipt>, CandidateSourceReclaimError> {
        let path = self.receipt_path(kind, subject_id);
        if !path_exists(&path)? {
            return Ok(None);
        }
        let text = read_protected_text(
            &path,
            self.expected_owner_uid,
            MAX_LIFECYCLE_RECORD_BYTES,
        )?;
        let receipt = parse_lifecycle_receipt(&text)?;
        if receipt.kind != kind || receipt.subject_id != subject_id {
            return Err(CandidateSourceReclaimError::InvalidLifecycleReceipt(
                path.display().to_string(),
            ));
        }
        Ok(Some(receipt))
    }

    fn create_source_retirement_receipt(
        &self,
        manifest_id: &str,
        source: &Path,
        now_unix_ms: u64,
    ) -> Result<(), CandidateSourceReclaimError> {
        let path = self.receipt_path(SOURCE_RETIREMENT, manifest_id);
        let count = fs::read_dir(self.receipt_root())
            .map_err(io_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(io_error)?
            .len();
        if count >= MAX_LIFECYCLE_RECEIPTS {
            return Err(CandidateSourceReclaimError::ReceiptCapacityExceeded);
        }
        let evidence = [
            format!("source:{}", source.display()),
            "decision:manifest-retired-lease-and-job-drain-proved".to_owned(),
        ];
        let text = canonical_lifecycle_receipt(
            SOURCE_RETIREMENT,
            manifest_id,
            now_unix_ms,
            None,
            &evidence,
        );
        create_protected_file(&path, &text)?;
        sync_dir(&self.receipt_root())
    }

    fn receipt_root(&self) -> PathBuf {
        self.paths.lifecycle_root.join(RECEIPTS)
    }

    fn receipt_path(&self, kind: &str, subject_id: &str) -> PathBuf {
        self.receipt_root()
            .join(format!("{}-{}.receipt", kind, hex_text(subject_id)))
    }

    fn candidate_manifest_path(&self, manifest_id: &str) -> PathBuf {
        self.paths.candidate_root.join(format!(
            "manifest-{}.candidate",
            hex_text(manifest_id)
        ))
    }

    fn producer_receipt_path(&self, manifest_id: &str) -> PathBuf {
        self.paths.producer_receipt_root.join(format!(
            "manifest-{}.receipt",
            hex_text(manifest_id)
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LifecycleReceipt {
    kind: String,
    subject_id: String,
    occurred_at_unix_ms: u64,
    request_id: String,
    manifest_id: String,
    operation: String,
    evidence: Vec<String>,
}

fn parse_lifecycle_receipt(
    text: &str,
) -> Result<LifecycleReceipt, CandidateSourceReclaimError> {
    if text.len() as u64 > MAX_LIFECYCLE_RECORD_BYTES {
        return Err(CandidateSourceReclaimError::InvalidLifecycleReceipt(
            "receipt too large".into(),
        ));
    }
    let mut lines = text.split('\n');
    if lines.next() != Some("theblob-materialization-lifecycle-receipt-v1") {
        return Err(CandidateSourceReclaimError::InvalidLifecycleReceipt(
            "header".into(),
        ));
    }
    let kind = canonical_hex_field(lines.next(), "kind")?;
    let subject_id = canonical_hex_field(lines.next(), "subject-id")?;
    let occurred_raw = plain_field(lines.next(), "occurred-at-unix-ms")?;
    let occurred_at_unix_ms = occurred_raw.parse::<u64>().map_err(|_| {
        CandidateSourceReclaimError::InvalidLifecycleReceipt("occurred-at-unix-ms".into())
    })?;
    if occurred_at_unix_ms.to_string() != occurred_raw {
        return Err(CandidateSourceReclaimError::InvalidLifecycleReceipt(
            "noncanonical occurred-at-unix-ms".into(),
        ));
    }
    let request_id = canonical_hex_field(lines.next(), "request-id")?;
    let manifest_id = canonical_hex_field(lines.next(), "manifest-id")?;
    let operation = canonical_hex_field(lines.next(), "operation")?;
    let evidence_count_raw = plain_field(lines.next(), "evidence-count")?;
    let evidence_count = evidence_count_raw.parse::<usize>().map_err(|_| {
        CandidateSourceReclaimError::InvalidLifecycleReceipt("evidence-count".into())
    })?;
    if evidence_count.to_string() != evidence_count_raw || evidence_count > 128 {
        return Err(CandidateSourceReclaimError::InvalidLifecycleReceipt(
            "noncanonical evidence-count".into(),
        ));
    }
    let mut evidence = Vec::with_capacity(evidence_count);
    for index in 0..evidence_count {
        evidence.push(canonical_hex_field(
            lines.next(),
            &format!("evidence-{index}"),
        )?);
    }
    if lines.next() != Some("") || lines.next().is_some() {
        return Err(CandidateSourceReclaimError::InvalidLifecycleReceipt(
            "trailing fields".into(),
        ));
    }
    Ok(LifecycleReceipt {
        kind,
        subject_id,
        occurred_at_unix_ms,
        request_id,
        manifest_id,
        operation,
        evidence,
    })
}

fn retirement_source(receipt: &LifecycleReceipt) -> Result<PathBuf, CandidateSourceReclaimError> {
    let matches = receipt
        .evidence
        .iter()
        .filter_map(|value| value.strip_prefix("source-retained:"))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(CandidateSourceReclaimError::InvalidLifecycleReceipt(
            "candidate retirement source evidence".into(),
        ));
    }
    Ok(PathBuf::from(matches[0]))
}

fn require_source_retirement_identity(
    receipt: &LifecycleReceipt,
    source: &Path,
) -> Result<(), CandidateSourceReclaimError> {
    let expected = format!("source:{}", source.display());
    if receipt.evidence.iter().filter(|item| **item == expected).count() != 1
        || !receipt
            .evidence
            .iter()
            .any(|item| item == "decision:manifest-retired-lease-and-job-drain-proved")
    {
        return Err(CandidateSourceReclaimError::InvalidLifecycleReceipt(
            "source retirement identity".into(),
        ));
    }
    Ok(())
}

fn validate_manifest_id(value: &str) -> Result<(), CandidateSourceReclaimError> {
    if !value.starts_with("manifest:")
        || value.len() > 256
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(CandidateSourceReclaimError::InvalidManifestId);
    }
    Ok(())
}

fn validate_directory(
    path: &Path,
    expected_owner_uid: u32,
) -> Result<(), CandidateSourceReclaimError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        CandidateSourceReclaimError::InvalidLayout(path.display().to_string())
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != expected_owner_uid
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(CandidateSourceReclaimError::InvalidLayout(
            path.display().to_string(),
        ));
    }
    Ok(())
}

fn read_protected_text(
    path: &Path,
    expected_owner_uid: u32,
    max_bytes: u64,
) -> Result<String, CandidateSourceReclaimError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != expected_owner_uid
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.len() > max_bytes
    {
        return Err(CandidateSourceReclaimError::InvalidLayout(
            path.display().to_string(),
        ));
    }
    let mut text = String::new();
    File::open(path)
        .and_then(|mut file| file.read_to_string(&mut text))
        .map_err(io_error)?;
    Ok(text)
}

fn create_protected_file(
    path: &Path,
    text: &str,
) -> Result<(), CandidateSourceReclaimError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(io_error)?;
    file.write_all(text.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(io_error)
}

fn require_symlink_target(
    path: &Path,
    expected: &Path,
) -> Result<(), CandidateSourceReclaimError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(CandidateSourceReclaimError::MissingSourceRoot)
        }
        Err(error) => return Err(io_error(error)),
    };
    if !metadata.file_type().is_symlink() || fs::read_link(path).map_err(io_error)? != expected {
        return Err(CandidateSourceReclaimError::SourceRootConflict);
    }
    Ok(())
}

fn validate_exact_store_path(path: &Path) -> Result<(), CandidateSourceReclaimError> {
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
        Err(CandidateSourceReclaimError::InvalidLifecycleReceipt(
            format!("invalid exact source path {}", path.display()),
        ))
    }
}

fn canonical_hex_field(
    line: Option<&str>,
    key: &str,
) -> Result<String, CandidateSourceReclaimError> {
    let encoded = plain_field(line, key)?;
    if encoded.len() % 2 != 0
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(CandidateSourceReclaimError::InvalidLifecycleReceipt(
            format!("noncanonical {key}"),
        ));
    }
    let bytes = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).map_err(|_| {
                CandidateSourceReclaimError::InvalidLifecycleReceipt(key.to_owned())
            })?;
            u8::from_str_radix(text, 16).map_err(|_| {
                CandidateSourceReclaimError::InvalidLifecycleReceipt(key.to_owned())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let decoded = String::from_utf8(bytes).map_err(|_| {
        CandidateSourceReclaimError::InvalidLifecycleReceipt(key.to_owned())
    })?;
    if hex_text(&decoded) != encoded {
        return Err(CandidateSourceReclaimError::InvalidLifecycleReceipt(
            format!("noncanonical {key}"),
        ));
    }
    Ok(decoded)
}

fn plain_field<'a>(
    line: Option<&'a str>,
    key: &str,
) -> Result<&'a str, CandidateSourceReclaimError> {
    line.and_then(|value| value.strip_prefix(&format!("{key}=")))
        .ok_or_else(|| CandidateSourceReclaimError::InvalidLifecycleReceipt(key.to_owned()))
}

fn sync_dir(path: &Path) -> Result<(), CandidateSourceReclaimError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(io_error)
}

fn path_exists(path: &Path) -> Result<bool, CandidateSourceReclaimError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(error)),
    }
}

fn hex_text(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn lease_error(error: CandidateEnqueueLeaseError) -> CandidateSourceReclaimError {
    CandidateSourceReclaimError::Lease(format!("{error:?}"))
}

fn io_error(error: std::io::Error) -> CandidateSourceReclaimError {
    CandidateSourceReclaimError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_receipt_parser_accepts_lifecycle_canonical_form() {
        let text = canonical_lifecycle_receipt(
            CANDIDATE_RETIREMENT,
            "manifest:test",
            42,
            None,
            &[
                "source-retained:/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-source".into(),
                "terminal-jobs:0".into(),
            ],
        );
        let receipt = parse_lifecycle_receipt(&text).unwrap();
        assert_eq!(receipt.kind, CANDIDATE_RETIREMENT);
        assert_eq!(receipt.subject_id, "manifest:test");
        assert_eq!(receipt.occurred_at_unix_ms, 42);
        assert_eq!(
            retirement_source(&receipt).unwrap(),
            PathBuf::from("/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-source")
        );
    }

    #[test]
    fn strict_receipt_parser_rejects_trailing_fields() {
        let text = canonical_lifecycle_receipt(
            CANDIDATE_RETIREMENT,
            "manifest:test",
            42,
            None,
            &["source-retained:/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-source".into()],
        ) + "extra=1\n";
        assert!(parse_lifecycle_receipt(&text).is_err());
    }
}
