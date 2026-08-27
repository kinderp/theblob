#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_CANDIDATE_LEASE_ROOT: &str = "/var/lib/theblob/candidate-enqueue-leases";
const ACTIVE: &str = "active";
const RETIRING: &str = "retiring";
const RETIRED: &str = "retired";
const MAX_RECORD_BYTES: u64 = 16 * 1024;

#[derive(Debug)]
pub enum CandidateLeaseError {
    InvalidLayout,
    InvalidManifestId,
    Retiring,
    Retired,
    Busy,
    Malformed,
    OwnerMismatch,
    Clock(String),
    RandomSource(String),
    Io(String),
}

pub struct CandidateEnqueueLeaseManager {
    root: PathBuf,
    expected_owner_uid: u32,
}

#[derive(Debug)]
pub struct CandidateEnqueueLease {
    path: PathBuf,
    active_dir: PathBuf,
    released: bool,
}

impl Drop for CandidateEnqueueLease {
    fn drop(&mut self) {
        if !self.released {
            let _ = fs::remove_file(&self.path);
            let _ = sync_dir(&self.active_dir);
        }
    }
}

impl CandidateEnqueueLease {
    pub fn release(mut self) -> Result<(), CandidateLeaseError> {
        if !self.released {
            match fs::remove_file(&self.path) {
                Ok(()) => sync_dir(&self.active_dir)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(io_error(error)),
            }
            self.released = true;
        }
        Ok(())
    }
}

impl CandidateEnqueueLeaseManager {
    pub fn production_default() -> Self {
        Self::new(DEFAULT_CANDIDATE_LEASE_ROOT, 0)
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

    pub fn acquire_enqueue(&self, manifest_id: &str) -> Result<CandidateEnqueueLease, CandidateLeaseError> {
        self.validate_layout()?;
        validate_manifest_id(manifest_id)?;
        let key = hex_text(manifest_id);
        if self.retired_path(&key).exists() {
            return Err(CandidateLeaseError::Retired);
        }
        if self.retiring_path(&key).exists() {
            return Err(CandidateLeaseError::Retiring);
        }

        for _ in 0..8 {
            let token = random_hex_128().map_err(CandidateLeaseError::RandomSource)?;
            let path = self.active_dir().join(format!("{key}--{token}.lease"));
            let body = format!(
                "theblob-candidate-enqueue-lease-v1\nmanifest-id:{manifest_id}\ncreated-at-unix-ms:{}\n",
                now_unix_ms().map_err(CandidateLeaseError::Clock)?
            );
            match create_protected_file(&path, &body) {
                Ok(()) => {
                    sync_dir(&self.active_dir())?;
                    // Critical recheck: a retirement barrier may have won after
                    // the first marker check but before this lease became durable.
                    // The enqueue must not touch candidate/source state until this
                    // second check succeeds.
                    if self.retiring_path(&key).exists() || self.retired_path(&key).exists() {
                        let _ = fs::remove_file(&path);
                        let _ = sync_dir(&self.active_dir());
                        return Err(CandidateLeaseError::Retiring);
                    }
                    return Ok(CandidateEnqueueLease {
                        path,
                        active_dir: self.active_dir(),
                        released: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(error)),
            }
        }
        Err(CandidateLeaseError::RandomSource("could not allocate lease token".into()))
    }

    /// Publish a durable one-way retirement barrier. Once present, new enqueue
    /// leases fail before candidate state can be read. The marker is intentionally
    /// retained across Busy results and crashes.
    pub fn begin_retirement(&self, manifest_id: &str) -> Result<(), CandidateLeaseError> {
        self.validate_layout()?;
        validate_manifest_id(manifest_id)?;
        let key = hex_text(manifest_id);
        if self.retired_path(&key).exists() {
            return Ok(());
        }
        let path = self.retiring_path(&key);
        if !path.exists() {
            let body = format!(
                "theblob-candidate-retirement-barrier-v1\nmanifest-id:{manifest_id}\ncreated-at-unix-ms:{}\n",
                now_unix_ms().map_err(CandidateLeaseError::Clock)?
            );
            match create_protected_file(&path, &body) {
                Ok(()) => sync_dir(&self.retiring_dir())?,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(io_error(error)),
            }
        }
        self.validate_marker(&path, manifest_id)?;
        Ok(())
    }

    /// Proves quiescence only after the retirement barrier is durable. Late
    /// enqueue attempts can create a lease file transiently, but their mandatory
    /// post-create marker recheck rejects them before candidate/source access.
    pub fn require_quiescent(&self, manifest_id: &str) -> Result<(), CandidateLeaseError> {
        self.validate_layout()?;
        validate_manifest_id(manifest_id)?;
        let key = hex_text(manifest_id);
        let barrier = self.retiring_path(&key);
        if !barrier.exists() && !self.retired_path(&key).exists() {
            return Err(CandidateLeaseError::Retiring);
        }
        if barrier.exists() {
            self.validate_marker(&barrier, manifest_id)?;
        }
        for path in self.active_paths()? {
            let text = read_protected_text(&path, self.expected_owner_uid)?;
            if text.lines().any(|line| line == format!("manifest-id:{manifest_id}")) {
                return Err(CandidateLeaseError::Busy);
            }
        }
        Ok(())
    }

    pub fn mark_retired(&self, manifest_id: &str) -> Result<(), CandidateLeaseError> {
        self.require_quiescent(manifest_id)?;
        let key = hex_text(manifest_id);
        let retiring = self.retiring_path(&key);
        let retired = self.retired_path(&key);
        if retired.exists() {
            self.validate_marker(&retired, manifest_id)?;
            return Ok(());
        }
        fs::rename(&retiring, &retired).map_err(io_error)?;
        sync_dir(&self.retiring_dir())?;
        sync_dir(&self.retired_dir())?;
        Ok(())
    }

    pub fn require_retired(&self, manifest_id: &str) -> Result<(), CandidateLeaseError> {
        self.validate_layout()?;
        let path = self.retired_path(&hex_text(manifest_id));
        if !path.exists() {
            return Err(CandidateLeaseError::Retiring);
        }
        self.validate_marker(&path, manifest_id)
    }

    /// Safe only when the caller has exclusive ownership of the enqueue daemon
    /// after the previous service control group is gone. It removes abandoned
    /// pre-publication leases; durable begin jobs are recovered independently.
    pub fn recover_abandoned_enqueue_leases(&self) -> Result<usize, CandidateLeaseError> {
        self.validate_layout()?;
        let mut removed = 0usize;
        for path in self.active_paths()? {
            read_protected_text(&path, self.expected_owner_uid)?;
            fs::remove_file(&path).map_err(io_error)?;
            removed += 1;
        }
        if removed > 0 {
            sync_dir(&self.active_dir())?;
        }
        Ok(removed)
    }

    fn validate_layout(&self) -> Result<(), CandidateLeaseError> {
        for path in [&self.root, &self.active_dir(), &self.retiring_dir(), &self.retired_dir()] {
            validate_directory(path, self.expected_owner_uid)?;
        }
        Ok(())
    }

    fn validate_marker(&self, path: &Path, manifest_id: &str) -> Result<(), CandidateLeaseError> {
        let text = read_protected_text(path, self.expected_owner_uid)?;
        if !text.lines().any(|line| line == format!("manifest-id:{manifest_id}")) {
            return Err(CandidateLeaseError::Malformed);
        }
        Ok(())
    }

    fn active_paths(&self) -> Result<Vec<PathBuf>, CandidateLeaseError> {
        let mut paths = fs::read_dir(self.active_dir())
            .map_err(io_error)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|v| v.to_str()) == Some("lease"))
            .collect::<Vec<_>>();
        paths.sort();
        Ok(paths)
    }

    fn active_dir(&self) -> PathBuf { self.root.join(ACTIVE) }
    fn retiring_dir(&self) -> PathBuf { self.root.join(RETIRING) }
    fn retired_dir(&self) -> PathBuf { self.root.join(RETIRED) }
    fn retiring_path(&self, key: &str) -> PathBuf { self.retiring_dir().join(format!("{key}.barrier")) }
    fn retired_path(&self, key: &str) -> PathBuf { self.retired_dir().join(format!("{key}.barrier")) }
}

fn validate_manifest_id(value: &str) -> Result<(), CandidateLeaseError> {
    if value.is_empty() || value.len() > 512 || value.contains('\n') || value.contains('\r') {
        return Err(CandidateLeaseError::InvalidManifestId);
    }
    Ok(())
}

fn validate_directory(path: &Path, owner: u32) -> Result<(), CandidateLeaseError> {
    let meta = fs::symlink_metadata(path).map_err(io_error)?;
    if !meta.is_dir() || meta.file_type().is_symlink() || meta.uid() != owner || meta.permissions().mode() & 0o777 != 0o700 {
        return Err(CandidateLeaseError::InvalidLayout);
    }
    Ok(())
}

fn create_protected_file(path: &Path, body: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?;
    file.write_all(body.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

fn read_protected_text(path: &Path, owner: u32) -> Result<String, CandidateLeaseError> {
    let meta = fs::symlink_metadata(path).map_err(io_error)?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.uid() != owner || meta.permissions().mode() & 0o777 != 0o600 || meta.len() > MAX_RECORD_BYTES {
        return Err(CandidateLeaseError::OwnerMismatch);
    }
    let mut text = String::new();
    File::open(path).map_err(io_error)?.take(MAX_RECORD_BYTES + 1).read_to_string(&mut text).map_err(io_error)?;
    if text.len() as u64 > MAX_RECORD_BYTES {
        return Err(CandidateLeaseError::Malformed);
    }
    Ok(text)
}

fn sync_dir(path: &Path) -> Result<(), CandidateLeaseError> {
    File::open(path).and_then(|f| f.sync_all()).map_err(io_error)
}

fn io_error(error: std::io::Error) -> CandidateLeaseError { CandidateLeaseError::Io(error.to_string()) }

fn now_unix_ms() -> Result<u64, String> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?;
    u64::try_from(duration.as_millis()).map_err(|_| "clock overflow".into())
}

fn random_hex_128() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom").map_err(|e| e.to_string())?.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn hex_text(value: &str) -> String {
    value.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}
