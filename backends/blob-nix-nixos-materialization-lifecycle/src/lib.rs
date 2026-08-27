#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{symlink, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use blob_core::{NodeId, SystemOperationId};
use blob_nix_nixos_candidate_producer::{
    DEFAULT_CANDIDATE_RECEIPT_ROOT, DEFAULT_CANDIDATE_SOURCE_GCROOT_ROOT,
};
use blob_nix_nixos_materialization_authority::{
    canonical_intent, parse_intent, MaterializationAuthorityError, MaterializationIntent,
    NixMaterializationInspector, RootMaterializationAdmissionAuthority,
};
use blob_nix_nixos_materialization_begin::{
    FileTrustedMaterializationCandidateStore, MaterializationBeginError,
    RootMaterializationBeginBoundary, TrustedMaterializationCandidate,
    DEFAULT_MATERIALIZATION_ADMISSION_ROOT, DEFAULT_MATERIALIZATION_INTENT_ROOT,
    DEFAULT_PENDING_GCROOT_ROOT, DEFAULT_TRUSTED_CANDIDATE_ROOT,
};
use blob_nix_nixos_materialization_begin_queue::{
    canonical_begin_job, parse_begin_job, MaterializationBeginJob, DEFAULT_BEGIN_JOB_ROOT,
};
use blob_nix_nixos_request_publisher::{
    canonical_materialization_admission, parse_materialization_admission, MaterializationAdmission,
};

pub const DEFAULT_ADMITTED_CLOSURE_GCROOT_ROOT: &str =
    "/nix/var/nix/gcroots/theblob-admitted-closures";
pub const DEFAULT_MATERIALIZATION_LIFECYCLE_ROOT: &str =
    "/var/lib/theblob/materialization-lifecycle";
pub const MAX_LIFECYCLE_RECORD_BYTES: u64 = 256 * 1024;
pub const MAX_LIFECYCLE_RECEIPTS: usize = 4096;

const QUEUED: &str = "queued";
const RUNNING: &str = "running";
const COMPLETED: &str = "completed";
const FAILED: &str = "failed";
const PENDING: &str = "pending";
const RECEIPTS: &str = "receipts";

#[derive(Debug)]
pub enum LifecycleError {
    InvalidLayout(String),
    InvalidRecord(String),
    NonCanonicalRecord(String),
    StateConflict(String),
    OwnerMismatch,
    UnsafeCancellation(String),
    UnsafeRetirement(String),
    MissingAdmittedClosureRoot(SystemOperationId),
    AdmittedClosureRootConflict(SystemOperationId),
    DerivationGcRootConflict(SystemOperationId),
    ReceiptCapacityExceeded,
    Authority(MaterializationAuthorityError),
    Begin(MaterializationBeginError),
    Io(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivationGcRootDisposition {
    Absent,
    RetainedForRecovery,
    ReleasedAfterAdmission,
    ReleasedAfterFailure,
    OrphanRetained,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BeginState {
    Queued,
    Running,
    Completed,
    Failed,
}

impl BeginState {
    fn name(self) -> &'static str {
        match self {
            Self::Queued => QUEUED,
            Self::Running => RUNNING,
            Self::Completed => COMPLETED,
            Self::Failed => FAILED,
        }
    }

    fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }
}

#[derive(Clone, Debug)]
struct ObservedBeginJob {
    state: BeginState,
    job: MaterializationBeginJob,
    path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct LifecyclePaths {
    pub begin_job_root: PathBuf,
    pub candidate_root: PathBuf,
    pub candidate_receipt_root: PathBuf,
    pub candidate_source_gcroot_root: PathBuf,
    pub intent_root: PathBuf,
    pub admission_root: PathBuf,
    pub derivation_gcroot_root: PathBuf,
    pub admitted_closure_gcroot_root: PathBuf,
    pub lifecycle_root: PathBuf,
}

impl LifecyclePaths {
    pub fn production_default() -> Self {
        Self {
            begin_job_root: PathBuf::from(DEFAULT_BEGIN_JOB_ROOT),
            candidate_root: PathBuf::from(DEFAULT_TRUSTED_CANDIDATE_ROOT),
            candidate_receipt_root: PathBuf::from(DEFAULT_CANDIDATE_RECEIPT_ROOT),
            candidate_source_gcroot_root: PathBuf::from(DEFAULT_CANDIDATE_SOURCE_GCROOT_ROOT),
            intent_root: PathBuf::from(DEFAULT_MATERIALIZATION_INTENT_ROOT),
            admission_root: PathBuf::from(DEFAULT_MATERIALIZATION_ADMISSION_ROOT),
            derivation_gcroot_root: PathBuf::from(DEFAULT_PENDING_GCROOT_ROOT),
            admitted_closure_gcroot_root: PathBuf::from(DEFAULT_ADMITTED_CLOSURE_GCROOT_ROOT),
            lifecycle_root: PathBuf::from(DEFAULT_MATERIALIZATION_LIFECYCLE_ROOT),
        }
    }
}

/// Completes one exact materialization only after rooting the already-realized
/// expected output. A failure after rooting intentionally leaks retention.
pub struct RootSafeMaterializationFinalizer {
    authority: RootMaterializationAdmissionAuthority,
    boundary: RootMaterializationBeginBoundary,
    closure_roots: PathBuf,
    expected_owner_uid: u32,
}

impl RootSafeMaterializationFinalizer {
    pub fn production_default(local_node: impl Into<NodeId>) -> Self {
        Self::new(
            local_node,
            DEFAULT_TRUSTED_CANDIDATE_ROOT,
            DEFAULT_MATERIALIZATION_INTENT_ROOT,
            DEFAULT_MATERIALIZATION_ADMISSION_ROOT,
            DEFAULT_PENDING_GCROOT_ROOT,
            DEFAULT_ADMITTED_CLOSURE_GCROOT_ROOT,
            0,
        )
    }

    pub fn new(
        local_node: impl Into<NodeId>,
        candidate_root: impl Into<PathBuf>,
        intent_root: impl Into<PathBuf>,
        admission_root: impl Into<PathBuf>,
        derivation_roots: impl Into<PathBuf>,
        closure_roots: impl Into<PathBuf>,
        expected_owner_uid: u32,
    ) -> Self {
        let intent_root = intent_root.into();
        let admission_root = admission_root.into();
        Self {
            authority: RootMaterializationAdmissionAuthority::new(
                intent_root.clone(),
                admission_root.clone(),
                expected_owner_uid,
            ),
            boundary: RootMaterializationBeginBoundary::new(
                local_node,
                candidate_root,
                intent_root,
                admission_root,
                derivation_roots,
                expected_owner_uid,
            ),
            closure_roots: closure_roots.into(),
            expected_owner_uid,
        }
    }

    pub fn complete<I: NixMaterializationInspector>(
        &self,
        operation: &SystemOperationId,
        inspector: &I,
    ) -> Result<MaterializationAdmission, LifecycleError> {
        let pending = self
            .authority
            .load_pending(operation)
            .map_err(LifecycleError::Authority)?;
        self.retain_closure(operation, &pending.expected_output)?;
        let admission = self
            .boundary
            .complete(operation, inspector)
            .map_err(LifecycleError::Begin)?;
        if admission.materialization_operation != *operation
            || Path::new(&admission.system_closure) != pending.expected_output.as_path()
        {
            return Err(LifecycleError::StateConflict(format!(
                "admission identity changed for {operation}"
            )));
        }
        require_symlink_target(
            &self.admitted_closure_gcroot_path(operation),
            Path::new(&admission.system_closure),
            LifecycleError::MissingAdmittedClosureRoot(operation.clone()),
            LifecycleError::AdmittedClosureRootConflict(operation.clone()),
        )?;
        Ok(admission)
    }

    pub fn admitted_closure_gcroot_path(&self, operation: &SystemOperationId) -> PathBuf {
        self.closure_roots.join(format!(
            "operation-{}-closure",
            hex_text(operation.as_str())
        ))
    }

    fn retain_closure(
        &self,
        operation: &SystemOperationId,
        closure: &Path,
    ) -> Result<(), LifecycleError> {
        validate_exact_store_output(closure)?;
        if !closure.exists() {
            return Err(LifecycleError::UnsafeRetirement(format!(
                "cannot retain unrealized closure {}",
                closure.display()
            )));
        }
        validate_directory(&self.closure_roots, self.expected_owner_uid)?;
        let root = self.admitted_closure_gcroot_path(operation);
        match fs::symlink_metadata(&root) {
            Ok(metadata) => {
                if !metadata.file_type().is_symlink()
                    || fs::read_link(&root).map_err(io_error)? != closure
                {
                    return Err(LifecycleError::AdmittedClosureRootConflict(operation.clone()));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                symlink(closure, &root).map_err(io_error)?;
                sync_dir(&self.closure_roots)?;
            }
            Err(error) => return Err(io_error(error)),
        }
        Ok(())
    }
}

/// Conservative lifecycle operations. Unknown or ambiguous liveness is retained.
pub struct RootMaterializationLifecycleManager {
    paths: LifecyclePaths,
    candidates: FileTrustedMaterializationCandidateStore,
    expected_owner_uid: u32,
}

impl RootMaterializationLifecycleManager {
    pub fn production_default() -> Self {
        Self::new(LifecyclePaths::production_default(), 0)
    }

    pub fn new(paths: LifecyclePaths, expected_owner_uid: u32) -> Self {
        let candidates = FileTrustedMaterializationCandidateStore::new(
            paths.candidate_root.clone(),
            expected_owner_uid,
        );
        Self {
            paths,
            candidates,
            expected_owner_uid,
        }
    }

    pub fn cancel_queued(
        &self,
        request_id: &str,
        requester_uid: u32,
        now_unix_ms: u64,
    ) -> Result<(), LifecycleError> {
        self.validate_layout()?;
        let observed = self.find_job_by_request(request_id)?;
        if observed.job.requester_uid != requester_uid {
            return Err(LifecycleError::OwnerMismatch);
        }
        if observed.state != BeginState::Queued {
            return Err(LifecycleError::UnsafeCancellation(format!(
                "request {request_id} is {}, not queued",
                observed.state.name()
            )));
        }
        self.require_no_native_state(&observed.job.operation)?;
        self.ensure_receipt(
            "queued-cancel",
            request_id,
            now_unix_ms,
            Some(&observed.job),
            &["decision:cancel-before-worker-claim".into()],
        )?;
        self.move_queued_to_failed(&observed)
    }

    pub fn expire_queued(
        &self,
        request_id: &str,
        now_unix_ms: u64,
        stale_after_ms: u64,
    ) -> Result<(), LifecycleError> {
        self.validate_layout()?;
        let observed = self.find_job_by_request(request_id)?;
        if observed.state != BeginState::Queued {
            return Err(LifecycleError::UnsafeCancellation(format!(
                "request {request_id} is {}, not queued",
                observed.state.name()
            )));
        }
        if !age_at_least(now_unix_ms, observed.job.enqueued_at_unix_ms, stale_after_ms) {
            return Err(LifecycleError::UnsafeCancellation(format!(
                "request {request_id} is not stale"
            )));
        }
        self.require_no_native_state(&observed.job.operation)?;
        self.ensure_receipt(
            "queued-expiry",
            request_id,
            now_unix_ms,
            Some(&observed.job),
            &["decision:expire-before-worker-claim".into()],
        )?;
        self.move_queued_to_failed(&observed)
    }

    pub fn reconcile_derivation_gcroot(
        &self,
        operation: &SystemOperationId,
        now_unix_ms: u64,
    ) -> Result<DerivationGcRootDisposition, LifecycleError> {
        self.validate_layout()?;
        let root = self.derivation_gcroot_path(operation);
        let root_exists = path_exists(&root)?;

        if let Some(intent) = self.load_intent(PENDING, operation)? {
            require_symlink_target(
                &root,
                &intent.derivation_path,
                LifecycleError::DerivationGcRootConflict(operation.clone()),
                LifecycleError::DerivationGcRootConflict(operation.clone()),
            )?;
            return Ok(DerivationGcRootDisposition::RetainedForRecovery);
        }

        if let Some(admission) = self.load_admission(operation)? {
            let completed = self.load_intent(COMPLETED, operation)?.ok_or_else(|| {
                LifecycleError::UnsafeRetirement(format!(
                    "admission {operation} has no completed materialization intent"
                ))
            })?;
            if completed.expected_output != Path::new(&admission.system_closure) {
                return Err(LifecycleError::StateConflict(format!(
                    "completed intent and admission disagree for {operation}"
                )));
            }
            require_symlink_target(
                &self.admitted_closure_gcroot_path(operation),
                Path::new(&admission.system_closure),
                LifecycleError::MissingAdmittedClosureRoot(operation.clone()),
                LifecycleError::AdmittedClosureRootConflict(operation.clone()),
            )?;
            if !root_exists {
                return Ok(DerivationGcRootDisposition::Absent);
            }
            require_symlink_target(
                &root,
                &completed.derivation_path,
                LifecycleError::DerivationGcRootConflict(operation.clone()),
                LifecycleError::DerivationGcRootConflict(operation.clone()),
            )?;
            let observed = self.find_job_by_operation(operation)?;
            self.ensure_receipt(
                "derivation-release",
                operation.as_str(),
                now_unix_ms,
                observed.as_ref().map(|value| &value.job),
                &["decision:admission-and-closure-root-durable".into()],
            )?;
            fs::remove_file(&root).map_err(io_error)?;
            sync_dir(&self.paths.derivation_gcroot_root)?;
            return Ok(DerivationGcRootDisposition::ReleasedAfterAdmission);
        }

        match self.find_job_by_operation(operation)? {
            Some(observed) if observed.state == BeginState::Failed => {
                if self.load_intent(COMPLETED, operation)?.is_some() {
                    return Err(LifecycleError::UnsafeRetirement(format!(
                        "failed job {operation} has completed intent without admission"
                    )));
                }
                if !root_exists {
                    return Ok(DerivationGcRootDisposition::Absent);
                }
                require_derivation_symlink(&root, operation)?;
                self.ensure_receipt(
                    "derivation-release",
                    operation.as_str(),
                    now_unix_ms,
                    Some(&observed.job),
                    &["decision:failed-job-without-native-state".into()],
                )?;
                fs::remove_file(&root).map_err(io_error)?;
                sync_dir(&self.paths.derivation_gcroot_root)?;
                Ok(DerivationGcRootDisposition::ReleasedAfterFailure)
            }
            Some(_) if root_exists => Ok(DerivationGcRootDisposition::RetainedForRecovery),
            Some(_) => Ok(DerivationGcRootDisposition::Absent),
            None if root_exists => Ok(DerivationGcRootDisposition::OrphanRetained),
            None => Ok(DerivationGcRootDisposition::Absent),
        }
    }

    pub fn retire_candidate(
        &self,
        manifest_id: &str,
        now_unix_ms: u64,
        retention_ms: u64,
    ) -> Result<(), LifecycleError> {
        self.validate_layout()?;
        let candidate = self
            .candidates
            .load(manifest_id)
            .map_err(|error| LifecycleError::InvalidRecord(format!("{error:?}")))?;
        let jobs = self.jobs_for_manifest(manifest_id)?;
        if jobs.is_empty() {
            return Err(LifecycleError::UnsafeRetirement(format!(
                "unused candidate retirement is not proved: {manifest_id}"
            )));
        }
        if jobs.iter().any(|value| !value.state.terminal()) {
            return Err(LifecycleError::UnsafeRetirement(format!(
                "candidate {manifest_id} still has queued/running work"
            )));
        }
        let newest_enqueue = jobs
            .iter()
            .map(|value| value.job.enqueued_at_unix_ms)
            .max()
            .unwrap_or(now_unix_ms);
        if !age_at_least(now_unix_ms, newest_enqueue, retention_ms) {
            return Err(LifecycleError::UnsafeRetirement(format!(
                "candidate {manifest_id} retention window is still open"
            )));
        }
        for observed in &jobs {
            self.require_terminal_job_reclaimable(observed)?;
        }
        self.validate_candidate_retirement_inputs(&candidate)?;
        self.ensure_receipt(
            "candidate-retirement",
            manifest_id,
            now_unix_ms,
            jobs.first().map(|value| &value.job),
            &[
                format!("source:{}", candidate.immutable_flake_root.display()),
                format!("terminal-jobs:{}", jobs.len()),
            ],
        )?;

        // Source retention is removed last. A crash can leak retention but cannot
        // deliberately leave a selectable manifest pointing at an unrooted source.
        fs::remove_file(self.candidate_manifest_path(manifest_id)).map_err(io_error)?;
        sync_dir(&self.paths.candidate_root)?;
        fs::remove_file(self.candidate_producer_receipt_path(manifest_id)).map_err(io_error)?;
        sync_dir(&self.paths.candidate_receipt_root)?;
        fs::remove_file(self.candidate_source_gcroot_path(manifest_id)).map_err(io_error)?;
        sync_dir(&self.paths.candidate_source_gcroot_root)
    }

    pub fn retire_terminal_job(
        &self,
        request_id: &str,
        now_unix_ms: u64,
        retention_ms: u64,
    ) -> Result<(), LifecycleError> {
        self.validate_layout()?;
        let observed = self.find_job_by_request(request_id)?;
        if !observed.state.terminal() {
            return Err(LifecycleError::UnsafeRetirement(format!(
                "request {request_id} is not terminal"
            )));
        }
        if !age_at_least(now_unix_ms, observed.job.enqueued_at_unix_ms, retention_ms) {
            return Err(LifecycleError::UnsafeRetirement(format!(
                "request {request_id} retention window is still open"
            )));
        }
        if path_exists(&self.candidate_manifest_path(&observed.job.manifest_id))? {
            return Err(LifecycleError::UnsafeRetirement(format!(
                "candidate {} is still selectable",
                observed.job.manifest_id
            )));
        }
        self.require_receipt("candidate-retirement", &observed.job.manifest_id)?;
        self.require_terminal_job_reclaimable(&observed)?;
        self.ensure_receipt(
            "terminal-job-retirement",
            request_id,
            now_unix_ms,
            Some(&observed.job),
            &[format!("terminal-state:{}", observed.state.name())],
        )?;
        fs::remove_file(&observed.path).map_err(io_error)?;
        sync_dir(&self.paths.begin_job_root.join(observed.state.name()))
    }

    pub fn admitted_closure_gcroot_path(&self, operation: &SystemOperationId) -> PathBuf {
        self.paths.admitted_closure_gcroot_root.join(format!(
            "operation-{}-closure",
            hex_text(operation.as_str())
        ))
    }

    fn validate_layout(&self) -> Result<(), LifecycleError> {
        for path in [
            &self.paths.begin_job_root,
            &self.paths.candidate_root,
            &self.paths.candidate_receipt_root,
            &self.paths.candidate_source_gcroot_root,
            &self.paths.intent_root,
            &self.paths.admission_root,
            &self.paths.derivation_gcroot_root,
            &self.paths.admitted_closure_gcroot_root,
            &self.paths.lifecycle_root,
        ] {
            validate_directory(path, self.expected_owner_uid)?;
        }
        validate_directory(&self.receipt_root(), self.expected_owner_uid)?;
        for state in [QUEUED, RUNNING, COMPLETED, FAILED] {
            validate_directory(
                &self.paths.begin_job_root.join(state),
                self.expected_owner_uid,
            )?;
        }
        for state in [PENDING, COMPLETED] {
            validate_directory(&self.paths.intent_root.join(state), self.expected_owner_uid)?;
        }
        Ok(())
    }

    fn move_queued_to_failed(&self, observed: &ObservedBeginJob) -> Result<(), LifecycleError> {
        let failed = self.job_path(BeginState::Failed, &observed.job.request_id);
        if path_exists(&failed)? {
            return Err(LifecycleError::StateConflict(observed.job.request_id.clone()));
        }
        match fs::rename(&observed.path, &failed) {
            Ok(()) => {
                sync_dir(&self.paths.begin_job_root.join(QUEUED))?;
                sync_dir(&self.paths.begin_job_root.join(FAILED))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(LifecycleError::UnsafeCancellation(format!(
                    "request {} was claimed concurrently",
                    observed.job.request_id
                )))
            }
            Err(error) => Err(io_error(error)),
        }
    }

    fn require_no_native_state(&self, operation: &SystemOperationId) -> Result<(), LifecycleError> {
        if self.load_intent(PENDING, operation)?.is_some()
            || self.load_intent(COMPLETED, operation)?.is_some()
            || self.load_admission(operation)?.is_some()
            || path_exists(&self.derivation_gcroot_path(operation))?
        {
            return Err(LifecycleError::UnsafeCancellation(format!(
                "operation {operation} already has native durable state"
            )));
        }
        Ok(())
    }

    fn require_terminal_job_reclaimable(
        &self,
        observed: &ObservedBeginJob,
    ) -> Result<(), LifecycleError> {
        let operation = &observed.job.operation;
        match observed.state {
            BeginState::Completed => {
                if self.load_intent(PENDING, operation)?.is_some() {
                    return Err(LifecycleError::UnsafeRetirement(format!(
                        "completed begin job {operation} still has pending intent"
                    )));
                }
                let completed = self.load_intent(COMPLETED, operation)?.ok_or_else(|| {
                    LifecycleError::UnsafeRetirement(format!(
                        "completed begin job {operation} has no completed intent"
                    ))
                })?;
                let admission = self.load_admission(operation)?.ok_or_else(|| {
                    LifecycleError::UnsafeRetirement(format!(
                        "completed begin job {operation} has no admission"
                    ))
                })?;
                if completed.expected_output != Path::new(&admission.system_closure) {
                    return Err(LifecycleError::StateConflict(format!(
                        "completed intent and admission disagree for {operation}"
                    )));
                }
                require_symlink_target(
                    &self.admitted_closure_gcroot_path(operation),
                    Path::new(&admission.system_closure),
                    LifecycleError::MissingAdmittedClosureRoot(operation.clone()),
                    LifecycleError::AdmittedClosureRootConflict(operation.clone()),
                )
            }
            BeginState::Failed => {
                if self.load_intent(PENDING, operation)?.is_some()
                    || self.load_intent(COMPLETED, operation)?.is_some()
                    || self.load_admission(operation)?.is_some()
                    || path_exists(&self.derivation_gcroot_path(operation))?
                {
                    return Err(LifecycleError::UnsafeRetirement(format!(
                        "failed begin job {operation} still has native durable state"
                    )));
                }
                Ok(())
            }
            _ => Err(LifecycleError::UnsafeRetirement(format!(
                "job {} is not terminal",
                observed.job.request_id
            ))),
        }
    }

    fn validate_candidate_retirement_inputs(
        &self,
        candidate: &TrustedMaterializationCandidate,
    ) -> Result<(), LifecycleError> {
        let receipt = read_protected_text(
            &self.candidate_producer_receipt_path(&candidate.manifest_id),
            self.expected_owner_uid,
        )?;
        require_candidate_receipt_identity(&receipt, candidate)?;
        require_symlink_target(
            &self.candidate_source_gcroot_path(&candidate.manifest_id),
            &candidate.immutable_flake_root,
            LifecycleError::UnsafeRetirement(format!(
                "candidate {} has no source GC root",
                candidate.manifest_id
            )),
            LifecycleError::UnsafeRetirement(format!(
                "candidate {} source GC root changed",
                candidate.manifest_id
            )),
        )
    }

    fn find_job_by_request(&self, request_id: &str) -> Result<ObservedBeginJob, LifecycleError> {
        let mut found = None;
        for state in [
            BeginState::Queued,
            BeginState::Running,
            BeginState::Completed,
            BeginState::Failed,
        ] {
            let path = self.job_path(state, request_id);
            if !path_exists(&path)? {
                continue;
            }
            let job = self.load_job_path(&path)?;
            if job.request_id != request_id || found.is_some() {
                return Err(LifecycleError::StateConflict(request_id.to_owned()));
            }
            found = Some(ObservedBeginJob { state, job, path });
        }
        found.ok_or_else(|| LifecycleError::InvalidRecord(format!("unknown request {request_id}")))
    }

    fn find_job_by_operation(
        &self,
        operation: &SystemOperationId,
    ) -> Result<Option<ObservedBeginJob>, LifecycleError> {
        let mut found = None;
        for state in [
            BeginState::Queued,
            BeginState::Running,
            BeginState::Completed,
            BeginState::Failed,
        ] {
            for path in self.job_paths(state)? {
                let job = self.load_job_path(&path)?;
                if job.operation != *operation {
                    continue;
                }
                if found.is_some() {
                    return Err(LifecycleError::StateConflict(format!(
                        "multiple begin jobs share operation {operation}"
                    )));
                }
                found = Some(ObservedBeginJob { state, job, path });
            }
        }
        Ok(found)
    }

    fn jobs_for_manifest(&self, manifest_id: &str) -> Result<Vec<ObservedBeginJob>, LifecycleError> {
        let mut found = Vec::new();
        for state in [
            BeginState::Queued,
            BeginState::Running,
            BeginState::Completed,
            BeginState::Failed,
        ] {
            for path in self.job_paths(state)? {
                let job = self.load_job_path(&path)?;
                if job.manifest_id == manifest_id {
                    found.push(ObservedBeginJob { state, job, path });
                }
            }
        }
        Ok(found)
    }

    fn job_paths(&self, state: BeginState) -> Result<Vec<PathBuf>, LifecycleError> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(self.paths.begin_job_root.join(state.name())).map_err(io_error)? {
            let path = entry.map_err(io_error)?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("job") {
                return Err(LifecycleError::InvalidRecord(path.display().to_string()));
            }
            paths.push(path);
        }
        paths.sort();
        Ok(paths)
    }

    fn load_job_path(&self, path: &Path) -> Result<MaterializationBeginJob, LifecycleError> {
        let text = read_protected_text(path, self.expected_owner_uid)?;
        let job = parse_begin_job(&text)
            .map_err(|error| LifecycleError::InvalidRecord(format!("{error:?}")))?;
        if canonical_begin_job(&job) != text {
            return Err(LifecycleError::NonCanonicalRecord(path.display().to_string()));
        }
        Ok(job)
    }

    fn load_intent(
        &self,
        state: &str,
        operation: &SystemOperationId,
    ) -> Result<Option<MaterializationIntent>, LifecycleError> {
        let path = self.intent_path(state, operation);
        if !path_exists(&path)? {
            return Ok(None);
        }
        let text = read_protected_text(&path, self.expected_owner_uid)?;
        let intent = parse_intent(&text).map_err(LifecycleError::Authority)?;
        if intent.materialization_operation != *operation || canonical_intent(&intent) != text {
            return Err(LifecycleError::NonCanonicalRecord(path.display().to_string()));
        }
        Ok(Some(intent))
    }

    fn load_admission(
        &self,
        operation: &SystemOperationId,
    ) -> Result<Option<MaterializationAdmission>, LifecycleError> {
        let path = self.admission_path(operation);
        if !path_exists(&path)? {
            return Ok(None);
        }
        let text = read_protected_text(&path, self.expected_owner_uid)?;
        let admission = parse_materialization_admission(&text)
            .map_err(|error| LifecycleError::InvalidRecord(format!("{error:?}")))?;
        if admission.materialization_operation != *operation
            || canonical_materialization_admission(&admission) != text
        {
            return Err(LifecycleError::NonCanonicalRecord(path.display().to_string()));
        }
        Ok(Some(admission))
    }

    fn ensure_receipt(
        &self,
        kind: &str,
        subject_id: &str,
        occurred_at_unix_ms: u64,
        job: Option<&MaterializationBeginJob>,
        evidence: &[String],
    ) -> Result<(), LifecycleError> {
        let path = self.receipt_path(kind, subject_id);
        if path_exists(&path)? {
            return self.require_receipt(kind, subject_id);
        }
        let count = fs::read_dir(self.receipt_root())
            .map_err(io_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(io_error)?
            .len();
        if count >= MAX_LIFECYCLE_RECEIPTS {
            return Err(LifecycleError::ReceiptCapacityExceeded);
        }
        create_protected_file(
            &path,
            &canonical_lifecycle_receipt(kind, subject_id, occurred_at_unix_ms, job, evidence),
        )?;
        sync_dir(&self.receipt_root())
    }

    fn require_receipt(&self, kind: &str, subject_id: &str) -> Result<(), LifecycleError> {
        let path = self.receipt_path(kind, subject_id);
        let text = read_protected_text(&path, self.expected_owner_uid)?;
        let prefix = format!(
            "theblob-materialization-lifecycle-receipt-v1\nkind={}\nsubject-id={}\n",
            hex_text(kind),
            hex_text(subject_id)
        );
        if !text.starts_with(&prefix) {
            return Err(LifecycleError::NonCanonicalRecord(path.display().to_string()));
        }
        Ok(())
    }

    fn receipt_root(&self) -> PathBuf {
        self.paths.lifecycle_root.join(RECEIPTS)
    }

    fn receipt_path(&self, kind: &str, subject_id: &str) -> PathBuf {
        self.receipt_root()
            .join(format!("{}-{}.receipt", kind, hex_text(subject_id)))
    }

    fn job_path(&self, state: BeginState, request_id: &str) -> PathBuf {
        self.paths.begin_job_root.join(state.name()).join(format!(
            "request-{}.job",
            hex_text(request_id)
        ))
    }

    fn intent_path(&self, state: &str, operation: &SystemOperationId) -> PathBuf {
        self.paths.intent_root.join(state).join(format!(
            "operation-{}.intent",
            hex_text(operation.as_str())
        ))
    }

    fn admission_path(&self, operation: &SystemOperationId) -> PathBuf {
        self.paths.admission_root.join(format!(
            "operation-{}.admission",
            hex_text(operation.as_str())
        ))
    }

    fn derivation_gcroot_path(&self, operation: &SystemOperationId) -> PathBuf {
        self.paths.derivation_gcroot_root.join(format!(
            "operation-{}-derivation",
            hex_text(operation.as_str())
        ))
    }

    fn admitted_closure_gcroot_path(&self, operation: &SystemOperationId) -> PathBuf {
        self.paths.admitted_closure_gcroot_root.join(format!(
            "operation-{}-closure",
            hex_text(operation.as_str())
        ))
    }

    fn candidate_manifest_path(&self, manifest_id: &str) -> PathBuf {
        self.paths.candidate_root.join(format!(
            "manifest-{}.candidate",
            hex_text(manifest_id)
        ))
    }

    fn candidate_producer_receipt_path(&self, manifest_id: &str) -> PathBuf {
        self.paths.candidate_receipt_root.join(format!(
            "manifest-{}.receipt",
            hex_text(manifest_id)
        ))
    }

    fn candidate_source_gcroot_path(&self, manifest_id: &str) -> PathBuf {
        self.paths.candidate_source_gcroot_root.join(format!(
            "manifest-{}-source",
            hex_text(manifest_id)
        ))
    }
}

pub fn canonical_lifecycle_receipt(
    kind: &str,
    subject_id: &str,
    occurred_at_unix_ms: u64,
    job: Option<&MaterializationBeginJob>,
    evidence: &[String],
) -> String {
    let mut lines = vec![
        "theblob-materialization-lifecycle-receipt-v1".to_owned(),
        format!("kind={}", hex_text(kind)),
        format!("subject-id={}", hex_text(subject_id)),
        format!("occurred-at-unix-ms={occurred_at_unix_ms}"),
        format!(
            "request-id={}",
            job.map(|value| hex_text(&value.request_id)).unwrap_or_default()
        ),
        format!(
            "manifest-id={}",
            job.map(|value| hex_text(&value.manifest_id)).unwrap_or_default()
        ),
        format!(
            "operation={}",
            job.map(|value| hex_text(value.operation.as_str())).unwrap_or_default()
        ),
        format!("evidence-count={}", evidence.len()),
    ];
    for (index, item) in evidence.iter().enumerate() {
        lines.push(format!("evidence-{index}={}", hex_text(item)));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn require_candidate_receipt_identity(
    text: &str,
    candidate: &TrustedMaterializationCandidate,
) -> Result<(), LifecycleError> {
    let required = [
        "theblob-candidate-manifest-receipt-v1".to_owned(),
        format!("manifest-id={}", hex_text(&candidate.manifest_id)),
        format!("candidate={}", hex_text(candidate.candidate.as_str())),
        format!("system-spec={}", hex_text(candidate.system_spec.as_str())),
        format!(
            "immutable-flake-root={}",
            hex_text(&candidate.immutable_flake_root.display().to_string())
        ),
    ];
    let lines = text.lines().collect::<Vec<_>>();
    if lines.first().copied() != Some(required[0].as_str())
        || required[1..]
            .iter()
            .any(|expected| !lines.iter().any(|line| *line == expected))
        || !text.ends_with('\n')
    {
        return Err(LifecycleError::NonCanonicalRecord(format!(
            "candidate producer receipt does not match {}",
            candidate.manifest_id
        )));
    }
    Ok(())
}

fn validate_directory(path: &Path, expected_owner_uid: u32) -> Result<(), LifecycleError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != expected_owner_uid
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(LifecycleError::InvalidLayout(path.display().to_string()));
    }
    Ok(())
}

fn read_protected_text(path: &Path, expected_owner_uid: u32) -> Result<String, LifecycleError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != expected_owner_uid
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.len() > MAX_LIFECYCLE_RECORD_BYTES
    {
        return Err(LifecycleError::InvalidRecord(path.display().to_string()));
    }
    let mut text = String::new();
    File::open(path)
        .and_then(|mut file| file.read_to_string(&mut text))
        .map_err(io_error)?;
    Ok(text)
}

fn create_protected_file(path: &Path, text: &str) -> Result<(), LifecycleError> {
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
    missing: LifecycleError,
    conflict: LifecycleError,
) -> Result<(), LifecycleError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Err(missing),
        Err(error) => return Err(io_error(error)),
    };
    if !metadata.file_type().is_symlink() || fs::read_link(path).map_err(io_error)? != expected {
        return Err(conflict);
    }
    Ok(())
}

fn require_derivation_symlink(
    path: &Path,
    operation: &SystemOperationId,
) -> Result<(), LifecycleError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if !metadata.file_type().is_symlink() {
        return Err(LifecycleError::DerivationGcRootConflict(operation.clone()));
    }
    let target = fs::read_link(path).map_err(io_error)?;
    validate_exact_derivation(&target)
        .map_err(|_| LifecycleError::DerivationGcRootConflict(operation.clone()))
}

fn validate_exact_derivation(path: &Path) -> Result<(), LifecycleError> {
    validate_exact_store_path(path)?;
    if path.extension().and_then(|value| value.to_str()) == Some("drv") {
        Ok(())
    } else {
        Err(LifecycleError::InvalidRecord(format!(
            "not a derivation: {}",
            path.display()
        )))
    }
}

fn validate_exact_store_output(path: &Path) -> Result<(), LifecycleError> {
    validate_exact_store_path(path)?;
    if path.extension().and_then(|value| value.to_str()) != Some("drv") {
        Ok(())
    } else {
        Err(LifecycleError::InvalidRecord(format!(
            "output is a derivation: {}",
            path.display()
        )))
    }
}

fn validate_exact_store_path(path: &Path) -> Result<(), LifecycleError> {
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
        Err(LifecycleError::InvalidRecord(format!(
            "invalid exact Nix store path {}",
            path.display()
        )))
    }
}

fn sync_dir(path: &Path) -> Result<(), LifecycleError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(io_error)
}

fn path_exists(path: &Path) -> Result<bool, LifecycleError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(error)),
    }
}

fn age_at_least(now: u64, created: u64, minimum: u64) -> bool {
    now.checked_sub(created)
        .map(|age| age >= minimum)
        .unwrap_or(false)
}

fn hex_text(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn io_error(error: std::io::Error) -> LifecycleError {
    LifecycleError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_does_not_embed_raw_identifiers() {
        let job = MaterializationBeginJob {
            request_id: "begin-request:one".into(),
            requester_uid: 1000,
            requester_system_bus_name: ":1.55".into(),
            manifest_id: "manifest:one".into(),
            operation: SystemOperationId::from("op:materialize-one"),
            enqueued_at_unix_ms: 10,
        };
        let receipt = canonical_lifecycle_receipt(
            "queued-cancel",
            &job.request_id,
            20,
            Some(&job),
            &["decision:cancel-before-worker-claim".into()],
        );
        assert!(receipt.starts_with("theblob-materialization-lifecycle-receipt-v1\n"));
        assert!(receipt.contains("occurred-at-unix-ms=20\n"));
        assert!(!receipt.contains("begin-request:one"));
        assert!(receipt.ends_with('\n'));
    }

    #[test]
    fn clock_rollback_never_makes_a_record_old() {
        assert!(!age_at_least(9, 10, 0));
        assert!(age_at_least(10, 10, 0));
        assert!(age_at_least(20, 10, 10));
    }
}
