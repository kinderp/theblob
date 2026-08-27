#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{symlink, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use blob_core::{NodeId, SystemOperationId};
use blob_nix_nixos_materialization_authority::{
    MaterializationAuthorityError, MaterializationIntent, MaterializationIntentSpec,
    NixMaterializationInspector, RootMaterializationAdmissionAuthority,
};
use blob_nix_nixos_materialization_begin::{
    FileTrustedMaterializationCandidateStore, TrustedMaterializationCandidate,
    DEFAULT_MATERIALIZATION_ADMISSION_ROOT, DEFAULT_MATERIALIZATION_INTENT_ROOT,
    DEFAULT_PENDING_GCROOT_ROOT, DEFAULT_TRUSTED_CANDIDATE_ROOT,
};

pub const DEFAULT_BEGIN_JOB_ROOT: &str = "/var/lib/theblob/materialization-begin-jobs";
pub const MAX_BEGIN_JOB_BYTES: u64 = 16 * 1024;

const QUEUED: &str = "queued";
const RUNNING: &str = "running";
const COMPLETED: &str = "completed";
const FAILED: &str = "failed";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterializationBeginJob {
    pub request_id: String,
    pub requester_uid: u32,
    pub requester_system_bus_name: String,
    pub manifest_id: String,
    pub operation: SystemOperationId,
    pub enqueued_at_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterializationBeginJobState {
    Queued,
    Running,
    Completed,
    Failed,
}

impl MaterializationBeginJobState {
    fn directory(self) -> &'static str {
        match self {
            Self::Queued => QUEUED,
            Self::Running => RUNNING,
            Self::Completed => COMPLETED,
            Self::Failed => FAILED,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterializationBeginJobStatus {
    pub state: MaterializationBeginJobState,
    pub job: MaterializationBeginJob,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MaterializationBeginQueueError {
    InvalidLayout,
    InvalidSender,
    InvalidRequestId,
    InvalidJobFile,
    JobTooLarge,
    Malformed,
    NonCanonical,
    Missing(String),
    OwnerMismatch,
    StateConflict,
    Manifest(String),
    RandomSource(String),
    Clock(String),
    Io(String),
}

pub struct FileMaterializationBeginQueue {
    root: PathBuf,
    candidate_store: FileTrustedMaterializationCandidateStore,
    expected_owner_uid: u32,
}

impl FileMaterializationBeginQueue {
    pub fn production_default() -> Self {
        Self::new(
            DEFAULT_BEGIN_JOB_ROOT,
            DEFAULT_TRUSTED_CANDIDATE_ROOT,
            0,
        )
    }

    pub fn new(
        root: impl Into<PathBuf>,
        candidate_root: impl Into<PathBuf>,
        expected_owner_uid: u32,
    ) -> Self {
        Self {
            root: root.into(),
            candidate_store: FileTrustedMaterializationCandidateStore::new(
                candidate_root,
                expected_owner_uid,
            ),
            expected_owner_uid,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Enqueue long-running materialization work without doing Nix evaluation.
    ///
    /// The manifest is validated before the durable job is published. Both the
    /// request id and materialization operation id are generated inside root so
    /// a later retry can reconcile exactly the same operation after a crash.
    pub fn enqueue(
        &self,
        requester_uid: u32,
        requester_system_bus_name: &str,
        manifest_id: &str,
    ) -> Result<MaterializationBeginJob, MaterializationBeginQueueError> {
        self.validate_layout()?;
        validate_sender(requester_system_bus_name)?;
        self.candidate_store
            .load(manifest_id)
            .map_err(|error| MaterializationBeginQueueError::Manifest(format!("{error:?}")))?;

        for _ in 0..8 {
            let request_id = format!(
                "begin-request:{}",
                random_hex_128().map_err(MaterializationBeginQueueError::RandomSource)?
            );
            let operation = SystemOperationId::from(format!(
                "op:materialize-{}",
                random_hex_128().map_err(MaterializationBeginQueueError::RandomSource)?
            ));
            let job = MaterializationBeginJob {
                request_id: request_id.clone(),
                requester_uid,
                requester_system_bus_name: requester_system_bus_name.to_owned(),
                manifest_id: manifest_id.to_owned(),
                operation,
                enqueued_at_unix_ms: now_unix_ms()
                    .map_err(MaterializationBeginQueueError::Clock)?,
            };
            let path = self.job_path(MaterializationBeginJobState::Queued, &request_id);
            match create_job_file(&path, &canonical_begin_job(&job)) {
                Ok(()) => {
                    sync_dir(&self.state_dir(MaterializationBeginJobState::Queued))?;
                    return Ok(job);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(MaterializationBeginQueueError::Io(error.to_string())),
            }
        }
        Err(MaterializationBeginQueueError::RandomSource(
            "could not allocate a unique begin request id".into(),
        ))
    }

    /// Claim one queued job by atomic rename. With one systemd-owned worker this
    /// is the execution lease; a racing second worker cannot rename the same file.
    pub fn claim_next(
        &self,
    ) -> Result<Option<MaterializationBeginJob>, MaterializationBeginQueueError> {
        self.validate_layout()?;
        let mut paths = fs::read_dir(self.state_dir(MaterializationBeginJobState::Queued))
            .map_err(|error| MaterializationBeginQueueError::Io(error.to_string()))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("job"))
            .collect::<Vec<_>>();
        paths.sort();

        for queued_path in paths {
            let job = self.read_job_path(&queued_path)?;
            self.reject_other_state(&job.request_id, MaterializationBeginJobState::Queued)?;
            let running_path = self.job_path(MaterializationBeginJobState::Running, &job.request_id);
            match fs::rename(&queued_path, &running_path) {
                Ok(()) => {
                    sync_dir(&self.state_dir(MaterializationBeginJobState::Queued))?;
                    sync_dir(&self.state_dir(MaterializationBeginJobState::Running))?;
                    return Ok(Some(job));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(MaterializationBeginQueueError::Io(error.to_string())),
            }
        }
        Ok(None)
    }

    /// A daemon calls this only after obtaining exclusive service ownership on
    /// startup. systemd's control-group kill semantics ensure the old worker and
    /// its Nix child are gone before stranded `running` jobs become eligible again.
    pub fn recover_running(&self) -> Result<usize, MaterializationBeginQueueError> {
        self.validate_layout()?;
        let running_dir = self.state_dir(MaterializationBeginJobState::Running);
        let mut paths = fs::read_dir(&running_dir)
            .map_err(|error| MaterializationBeginQueueError::Io(error.to_string()))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("job"))
            .collect::<Vec<_>>();
        paths.sort();

        let mut recovered = 0usize;
        for running_path in paths {
            let job = self.read_job_path(&running_path)?;
            if self.exists(MaterializationBeginJobState::Completed, &job.request_id)?
                || self.exists(MaterializationBeginJobState::Failed, &job.request_id)?
                || self.exists(MaterializationBeginJobState::Queued, &job.request_id)?
            {
                return Err(MaterializationBeginQueueError::StateConflict);
            }
            let queued_path = self.job_path(MaterializationBeginJobState::Queued, &job.request_id);
            fs::rename(&running_path, &queued_path)
                .map_err(|error| MaterializationBeginQueueError::Io(error.to_string()))?;
            recovered += 1;
        }
        if recovered > 0 {
            sync_dir(&running_dir)?;
            sync_dir(&self.state_dir(MaterializationBeginJobState::Queued))?;
        }
        Ok(recovered)
    }

    pub fn mark_completed(
        &self,
        request_id: &str,
    ) -> Result<(), MaterializationBeginQueueError> {
        self.mark_terminal(request_id, MaterializationBeginJobState::Completed)
    }

    pub fn mark_failed(
        &self,
        request_id: &str,
    ) -> Result<(), MaterializationBeginQueueError> {
        self.mark_terminal(request_id, MaterializationBeginJobState::Failed)
    }

    pub fn load_running(
        &self,
        request_id: &str,
    ) -> Result<MaterializationBeginJob, MaterializationBeginQueueError> {
        self.validate_layout()?;
        validate_request_id(request_id)?;
        self.read_job_path(&self.job_path(MaterializationBeginJobState::Running, request_id))
    }

    pub fn status_for_uid(
        &self,
        request_id: &str,
        requester_uid: u32,
    ) -> Result<MaterializationBeginJobStatus, MaterializationBeginQueueError> {
        self.validate_layout()?;
        validate_request_id(request_id)?;
        let mut observed = None;
        for state in [
            MaterializationBeginJobState::Queued,
            MaterializationBeginJobState::Running,
            MaterializationBeginJobState::Completed,
            MaterializationBeginJobState::Failed,
        ] {
            let path = self.job_path(state, request_id);
            if path_exists(&path)? {
                if observed.is_some() {
                    return Err(MaterializationBeginQueueError::StateConflict);
                }
                observed = Some((state, self.read_job_path(&path)?));
            }
        }
        let (state, job) = observed
            .ok_or_else(|| MaterializationBeginQueueError::Missing(request_id.to_owned()))?;
        if job.requester_uid != requester_uid {
            return Err(MaterializationBeginQueueError::OwnerMismatch);
        }
        Ok(MaterializationBeginJobStatus { state, job })
    }

    fn mark_terminal(
        &self,
        request_id: &str,
        terminal: MaterializationBeginJobState,
    ) -> Result<(), MaterializationBeginQueueError> {
        self.validate_layout()?;
        validate_request_id(request_id)?;
        if !matches!(
            terminal,
            MaterializationBeginJobState::Completed | MaterializationBeginJobState::Failed
        ) {
            return Err(MaterializationBeginQueueError::StateConflict);
        }
        let running = self.job_path(MaterializationBeginJobState::Running, request_id);
        let job = self.read_job_path(&running)?;
        self.reject_other_state(request_id, MaterializationBeginJobState::Running)?;
        let target = self.job_path(terminal, request_id);
        if path_exists(&target)? {
            return Err(MaterializationBeginQueueError::StateConflict);
        }
        fs::rename(&running, &target)
            .map_err(|error| MaterializationBeginQueueError::Io(error.to_string()))?;
        sync_dir(&self.state_dir(MaterializationBeginJobState::Running))?;
        sync_dir(&self.state_dir(terminal))?;
        let observed = self.read_job_path(&target)?;
        if observed != job {
            return Err(MaterializationBeginQueueError::StateConflict);
        }
        Ok(())
    }

    fn validate_layout(&self) -> Result<(), MaterializationBeginQueueError> {
        validate_directory(&self.root, self.expected_owner_uid)
            .map_err(|_| MaterializationBeginQueueError::InvalidLayout)?;
        for state in [
            MaterializationBeginJobState::Queued,
            MaterializationBeginJobState::Running,
            MaterializationBeginJobState::Completed,
            MaterializationBeginJobState::Failed,
        ] {
            validate_directory(&self.state_dir(state), self.expected_owner_uid)
                .map_err(|_| MaterializationBeginQueueError::InvalidLayout)?;
        }
        Ok(())
    }

    fn state_dir(&self, state: MaterializationBeginJobState) -> PathBuf {
        self.root.join(state.directory())
    }

    fn job_path(&self, state: MaterializationBeginJobState, request_id: &str) -> PathBuf {
        self.state_dir(state)
            .join(format!("request-{}.job", hex_text(request_id)))
    }

    fn exists(
        &self,
        state: MaterializationBeginJobState,
        request_id: &str,
    ) -> Result<bool, MaterializationBeginQueueError> {
        path_exists(&self.job_path(state, request_id))
    }

    fn reject_other_state(
        &self,
        request_id: &str,
        expected: MaterializationBeginJobState,
    ) -> Result<(), MaterializationBeginQueueError> {
        for state in [
            MaterializationBeginJobState::Queued,
            MaterializationBeginJobState::Running,
            MaterializationBeginJobState::Completed,
            MaterializationBeginJobState::Failed,
        ] {
            if state != expected && self.exists(state, request_id)? {
                return Err(MaterializationBeginQueueError::StateConflict);
            }
        }
        Ok(())
    }

    fn read_job_path(
        &self,
        path: &Path,
    ) -> Result<MaterializationBeginJob, MaterializationBeginQueueError> {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                MaterializationBeginQueueError::Missing(path.display().to_string())
            } else {
                MaterializationBeginQueueError::Io(error.to_string())
            }
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != self.expected_owner_uid
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(MaterializationBeginQueueError::InvalidJobFile);
        }
        if metadata.len() > MAX_BEGIN_JOB_BYTES {
            return Err(MaterializationBeginQueueError::JobTooLarge);
        }
        let mut text = String::new();
        File::open(path)
            .and_then(|mut file| file.read_to_string(&mut text))
            .map_err(|error| MaterializationBeginQueueError::Io(error.to_string()))?;
        let job = parse_begin_job(&text)?;
        if canonical_begin_job(&job) != text {
            return Err(MaterializationBeginQueueError::NonCanonical);
        }
        Ok(job)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoverableBeginError {
    Queue(MaterializationBeginQueueError),
    Manifest(String),
    InvalidImmutableSource,
    Authority(MaterializationAuthorityError),
    Inspector(String),
    InvalidGcRoot,
    GcRootConflict,
    IdentityMismatch,
    Clock(String),
    Io(String),
}

/// Root-side long-running coordinator used by the asynchronous worker.
///
/// The materialization operation id is already durable in the begin job before
/// Nix evaluation starts. Retrying the same job therefore reconciles one exact
/// operation instead of allocating a second privileged/native identity.
pub struct RecoverableMaterializationBeginCoordinator {
    local_node: NodeId,
    candidates: FileTrustedMaterializationCandidateStore,
    authority: RootMaterializationAdmissionAuthority,
    gcroot_root: PathBuf,
    expected_owner_uid: u32,
}

impl RecoverableMaterializationBeginCoordinator {
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

    pub fn start_or_reconcile<I: NixMaterializationInspector>(
        &self,
        job: &MaterializationBeginJob,
        inspector: &I,
    ) -> Result<MaterializationIntent, RecoverableBeginError> {
        let candidate = self
            .candidates
            .load(&job.manifest_id)
            .map_err(|error| RecoverableBeginError::Manifest(format!("{error:?}")))?;
        validate_canonical_immutable_store_subpath(&candidate.immutable_flake_root)
            .map_err(|_| RecoverableBeginError::InvalidImmutableSource)?;

        match self.authority.load_pending(&job.operation) {
            Ok(intent) => {
                self.validate_existing(job, &candidate, &intent)?;
                self.require_derivation(&job.operation, &intent.derivation_path)?;
                return Ok(intent);
            }
            Err(MaterializationAuthorityError::IntentMissing(_)) => {}
            Err(error) => return Err(RecoverableBeginError::Authority(error)),
        }

        let resolved = inspector
            .resolve_exact_derivation(
                &candidate.immutable_flake_root,
                &candidate.installable_attribute,
            )
            .map_err(RecoverableBeginError::Inspector)?;
        self.retain_derivation(&job.operation, &resolved.derivation_path)?;

        let begin_result = self.authority.begin(
            &MaterializationIntentSpec {
                node: self.local_node.clone(),
                candidate: candidate.candidate.clone(),
                system_spec: candidate.system_spec.clone(),
                materialization_operation: job.operation.clone(),
                immutable_flake_root: candidate.immutable_flake_root.clone(),
                installable_attribute: candidate.installable_attribute.clone(),
                created_at_unix_ms: now_unix_ms().map_err(RecoverableBeginError::Clock)?,
            },
            inspector,
        );

        let intent = match begin_result {
            Ok(intent) => intent,
            Err(error) => match self.authority.load_pending(&job.operation) {
                Ok(intent) => intent,
                Err(_) => {
                    let _ = self.release_derivation(&job.operation);
                    return Err(RecoverableBeginError::Authority(error));
                }
            },
        };
        self.validate_existing(job, &candidate, &intent)?;
        if intent.derivation_path != resolved.derivation_path
            || intent.expected_output != resolved.expected_output
        {
            return Err(RecoverableBeginError::IdentityMismatch);
        }
        self.require_derivation(&job.operation, &intent.derivation_path)?;
        Ok(intent)
    }

    fn validate_existing(
        &self,
        job: &MaterializationBeginJob,
        candidate: &TrustedMaterializationCandidate,
        intent: &MaterializationIntent,
    ) -> Result<(), RecoverableBeginError> {
        if intent.node != self.local_node
            || intent.materialization_operation != job.operation
            || intent.candidate != candidate.candidate
            || intent.system_spec != candidate.system_spec
            || intent.immutable_flake_root != candidate.immutable_flake_root
            || intent.installable_attribute != candidate.installable_attribute
        {
            return Err(RecoverableBeginError::IdentityMismatch);
        }
        Ok(())
    }

    fn validate_gcroot_root(&self) -> Result<(), RecoverableBeginError> {
        validate_directory(&self.gcroot_root, self.expected_owner_uid)
            .map_err(|_| RecoverableBeginError::InvalidGcRoot)
    }

    fn gcroot_path(&self, operation: &SystemOperationId) -> PathBuf {
        self.gcroot_root.join(format!(
            "operation-{}-derivation",
            hex_text(operation.as_str())
        ))
    }

    fn retain_derivation(
        &self,
        operation: &SystemOperationId,
        derivation: &Path,
    ) -> Result<(), RecoverableBeginError> {
        self.validate_gcroot_root()?;
        let root = self.gcroot_path(operation);
        match fs::symlink_metadata(&root) {
            Ok(metadata) => {
                if !metadata.file_type().is_symlink() {
                    return Err(RecoverableBeginError::GcRootConflict);
                }
                let target = fs::read_link(&root)
                    .map_err(|error| RecoverableBeginError::Io(error.to_string()))?;
                if target != derivation {
                    return Err(RecoverableBeginError::GcRootConflict);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                symlink(derivation, &root)
                    .map_err(|error| RecoverableBeginError::Io(error.to_string()))?;
                sync_dir(&self.gcroot_root)
                    .map_err(RecoverableBeginError::Queue)?;
            }
            Err(error) => return Err(RecoverableBeginError::Io(error.to_string())),
        }
        Ok(())
    }

    fn require_derivation(
        &self,
        operation: &SystemOperationId,
        derivation: &Path,
    ) -> Result<(), RecoverableBeginError> {
        self.validate_gcroot_root()?;
        let root = self.gcroot_path(operation);
        let metadata = fs::symlink_metadata(&root)
            .map_err(|error| RecoverableBeginError::Io(error.to_string()))?;
        if !metadata.file_type().is_symlink() {
            return Err(RecoverableBeginError::GcRootConflict);
        }
        let target = fs::read_link(&root)
            .map_err(|error| RecoverableBeginError::Io(error.to_string()))?;
        if target != derivation || !derivation.exists() {
            return Err(RecoverableBeginError::GcRootConflict);
        }
        Ok(())
    }

    fn release_derivation(
        &self,
        operation: &SystemOperationId,
    ) -> Result<(), RecoverableBeginError> {
        self.validate_gcroot_root()?;
        match fs::remove_file(self.gcroot_path(operation)) {
            Ok(()) => sync_dir(&self.gcroot_root)
                .map_err(RecoverableBeginError::Queue),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(RecoverableBeginError::Io(error.to_string())),
        }
    }
}

pub fn canonical_begin_job(job: &MaterializationBeginJob) -> String {
    [
        "theblob-materialization-begin-job-v1".to_owned(),
        format!("request-id={}", hex_text(&job.request_id)),
        format!("requester-uid={}", job.requester_uid),
        format!(
            "requester-system-bus={}",
            hex_text(&job.requester_system_bus_name)
        ),
        format!("manifest-id={}", hex_text(&job.manifest_id)),
        format!("operation={}", hex_text(job.operation.as_str())),
        format!("enqueued-at-unix-ms={}", job.enqueued_at_unix_ms),
        String::new(),
    ]
    .join("\n")
}

pub fn parse_begin_job(
    text: &str,
) -> Result<MaterializationBeginJob, MaterializationBeginQueueError> {
    if text.len() as u64 > MAX_BEGIN_JOB_BYTES {
        return Err(MaterializationBeginQueueError::JobTooLarge);
    }
    let mut cursor = Cursor::new(text);
    cursor.literal("theblob-materialization-begin-job-v1")?;
    let request_id = cursor.hex_field("request-id")?;
    let requester_uid = cursor.u32_field("requester-uid")?;
    let requester_system_bus_name = cursor.hex_field("requester-system-bus")?;
    let manifest_id = cursor.hex_field("manifest-id")?;
    let operation = SystemOperationId::from(cursor.hex_field("operation")?);
    let enqueued_at_unix_ms = cursor.u64_field("enqueued-at-unix-ms")?;
    cursor.finish()?;
    validate_request_id(&request_id)?;
    validate_sender(&requester_system_bus_name)?;
    if manifest_id.is_empty() || manifest_id.len() > 256 {
        return Err(MaterializationBeginQueueError::Malformed);
    }
    Ok(MaterializationBeginJob {
        request_id,
        requester_uid,
        requester_system_bus_name,
        manifest_id,
        operation,
        enqueued_at_unix_ms,
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

    fn next(&mut self) -> Result<&'a str, MaterializationBeginQueueError> {
        let value = self
            .lines
            .get(self.position)
            .copied()
            .ok_or(MaterializationBeginQueueError::Malformed)?;
        self.position += 1;
        Ok(value)
    }

    fn literal(&mut self, expected: &str) -> Result<(), MaterializationBeginQueueError> {
        if self.next()? != expected {
            return Err(MaterializationBeginQueueError::Malformed);
        }
        Ok(())
    }

    fn field(&mut self, key: &str) -> Result<&'a str, MaterializationBeginQueueError> {
        self.next()?
            .strip_prefix(&format!("{key}="))
            .ok_or(MaterializationBeginQueueError::Malformed)
    }

    fn hex_field(&mut self, key: &str) -> Result<String, MaterializationBeginQueueError> {
        decode_hex(self.field(key)?)
    }

    fn u32_field(&mut self, key: &str) -> Result<u32, MaterializationBeginQueueError> {
        self.field(key)?
            .parse::<u32>()
            .map_err(|_| MaterializationBeginQueueError::Malformed)
    }

    fn u64_field(&mut self, key: &str) -> Result<u64, MaterializationBeginQueueError> {
        self.field(key)?
            .parse::<u64>()
            .map_err(|_| MaterializationBeginQueueError::Malformed)
    }

    fn finish(&mut self) -> Result<(), MaterializationBeginQueueError> {
        if !self.next()?.is_empty() || self.position != self.lines.len() {
            return Err(MaterializationBeginQueueError::Malformed);
        }
        Ok(())
    }
}

fn validate_request_id(value: &str) -> Result<(), MaterializationBeginQueueError> {
    if !value.starts_with("begin-request:")
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_'))
    {
        return Err(MaterializationBeginQueueError::InvalidRequestId);
    }
    Ok(())
}

fn validate_sender(value: &str) -> Result<(), MaterializationBeginQueueError> {
    if !value.starts_with(':')
        || value.len() > 256
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'.' | b'-' | b'_')
        })
    {
        return Err(MaterializationBeginQueueError::InvalidSender);
    }
    Ok(())
}

fn validate_directory(path: &Path, expected_owner_uid: u32) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != expected_owner_uid
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("invalid protected directory".into());
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
        return Err("source is outside an immutable Nix store object".into());
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

fn create_job_file(path: &Path, text: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

fn sync_dir(path: &Path) -> Result<(), MaterializationBeginQueueError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| MaterializationBeginQueueError::Io(error.to_string()))
}

fn path_exists(path: &Path) -> Result<bool, MaterializationBeginQueueError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(MaterializationBeginQueueError::Io(error.to_string())),
    }
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
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn decode_hex(value: &str) -> Result<String, MaterializationBeginQueueError> {
    if value.len() % 2 != 0 {
        return Err(MaterializationBeginQueueError::Malformed);
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high = nibble(pair[0])?;
        let low = nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }
    String::from_utf8(bytes).map_err(|_| MaterializationBeginQueueError::Malformed)
}

fn nibble(value: u8) -> Result<u8, MaterializationBeginQueueError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(MaterializationBeginQueueError::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> MaterializationBeginJob {
        MaterializationBeginJob {
            request_id: "begin-request:abc123".into(),
            requester_uid: 1000,
            requester_system_bus_name: ":1.42".into(),
            manifest_id: "manifest:one".into(),
            operation: SystemOperationId::from("op:materialize-one"),
            enqueued_at_unix_ms: 42,
        }
    }

    #[test]
    fn begin_job_round_trips_canonically() {
        let expected = job();
        let text = canonical_begin_job(&expected);
        assert_eq!(parse_begin_job(&text), Ok(expected));
    }

    #[test]
    fn request_id_rejects_path_and_shell_syntax() {
        assert!(validate_request_id("../begin-request:x").is_err());
        assert!(validate_request_id("begin-request:x;evil").is_err());
    }

    #[test]
    fn sender_requires_unique_bus_name_shape() {
        assert!(validate_sender(":1.42").is_ok());
        assert!(validate_sender("org.example.Service").is_err());
    }
}
