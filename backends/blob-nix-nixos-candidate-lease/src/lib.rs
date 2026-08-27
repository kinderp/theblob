#![forbid(unsafe_code)]

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_CANDIDATE_LEASE_ROOT: &str = "/var/lib/theblob/candidate-enqueue-leases";
const ACTIVE: &str = "active";
const RETIRING: &str = "retiring";
const RETIRED: &str = "retired";
const LEASE_VERSION: &str = "theblob-candidate-enqueue-lease-v1";
const BARRIER_VERSION: &str = "theblob-candidate-retirement-barrier-v1";
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
        if self.released {
            return Ok(());
        }
        match fs::remove_file(&self.path) {
            Ok(()) => sync_dir(&self.active_dir)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(error)),
        }
        self.released = true;
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

    /// Root may create only this dedicated subtree and only below an already
    /// trusted owner-only parent. Existing paths are never followed through a
    /// symlink and must have exact ownership and mode.
    pub fn prepare_layout(&self) -> Result<(), CandidateLeaseError> {
        let parent = self.root.parent().ok_or(CandidateLeaseError::InvalidLayout)?;
        validate_directory(parent, self.expected_owner_uid)?;
        create_or_validate_directory(&self.root, self.expected_owner_uid)?;
        create_or_validate_directory(&self.active_dir(), self.expected_owner_uid)?;
        create_or_validate_directory(&self.retiring_dir(), self.expected_owner_uid)?;
        create_or_validate_directory(&self.retired_dir(), self.expected_owner_uid)?;
        Ok(())
    }

    pub fn acquire_enqueue(
        &self,
        manifest_id: &str,
    ) -> Result<CandidateEnqueueLease, CandidateLeaseError> {
        self.prepare_layout()?;
        validate_manifest_id(manifest_id)?;
        let key = hex_text(manifest_id);
        self.reject_if_barred(&key, manifest_id)?;

        for _ in 0..8 {
            let token = random_hex_128().map_err(CandidateLeaseError::RandomSource)?;
            let path = self.active_dir().join(format!("{key}--{token}.lease"));
            let created_at = now_unix_ms().map_err(CandidateLeaseError::Clock)?;
            let body = canonical_record(LEASE_VERSION, manifest_id, created_at);
            match create_protected_file(&path, &body) {
                Ok(()) => {
                    sync_dir(&self.active_dir())?;
                    validate_record(
                        &path,
                        self.expected_owner_uid,
                        LEASE_VERSION,
                        manifest_id,
                    )?;

                    // Critical recheck: retirement may have won between the
                    // first barrier check and durable lease publication. The
                    // enqueue must not read candidate/source state until this
                    // second check succeeds.
                    if let Err(error) = self.reject_if_barred(&key, manifest_id) {
                        let _ = fs::remove_file(&path);
                        let _ = sync_dir(&self.active_dir());
                        return Err(error);
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
        Err(CandidateLeaseError::RandomSource(
            "could not allocate lease token".into(),
        ))
    }

    /// Publish a durable one-way retirement barrier. The barrier remains across
    /// Busy results and crashes, so candidate selection can never silently reopen.
    pub fn begin_retirement(&self, manifest_id: &str) -> Result<(), CandidateLeaseError> {
        self.prepare_layout()?;
        validate_manifest_id(manifest_id)?;
        let key = hex_text(manifest_id);
        let retired = self.retired_path(&key);
        if path_present(&retired)? {
            validate_record(
                &retired,
                self.expected_owner_uid,
                BARRIER_VERSION,
                manifest_id,
            )?;
            return Ok(());
        }

        let retiring = self.retiring_path(&key);
        if !path_present(&retiring)? {
            let created_at = now_unix_ms().map_err(CandidateLeaseError::Clock)?;
            let body = canonical_record(BARRIER_VERSION, manifest_id, created_at);
            match create_protected_file(&retiring, &body) {
                Ok(()) => sync_dir(&self.retiring_dir())?,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(io_error(error)),
            }
        }
        validate_record(
            &retiring,
            self.expected_owner_uid,
            BARRIER_VERSION,
            manifest_id,
        )
    }

    /// Quiescence is meaningful only after a durable barrier exists. Any active
    /// lease for this manifest makes retirement retain the source and return Busy.
    pub fn require_quiescent(&self, manifest_id: &str) -> Result<(), CandidateLeaseError> {
        self.prepare_layout()?;
        validate_manifest_id(manifest_id)?;
        let key = hex_text(manifest_id);
        let retiring = self.retiring_path(&key);
        let retired = self.retired_path(&key);
        let retiring_present = path_present(&retiring)?;
        let retired_present = path_present(&retired)?;
        if !retiring_present && !retired_present {
            return Err(CandidateLeaseError::Retiring);
        }
        if retiring_present {
            validate_record(
                &retiring,
                self.expected_owner_uid,
                BARRIER_VERSION,
                manifest_id,
            )?;
        }
        if retired_present {
            validate_record(
                &retired,
                self.expected_owner_uid,
                BARRIER_VERSION,
                manifest_id,
            )?;
        }

        for path in self.active_paths()? {
            let record = parse_record(
                &read_protected_text(&path, self.expected_owner_uid)?,
                LEASE_VERSION,
            )?;
            if record.manifest_id == manifest_id {
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
        if path_present(&retired)? {
            return validate_record(
                &retired,
                self.expected_owner_uid,
                BARRIER_VERSION,
                manifest_id,
            );
        }
        validate_record(
            &retiring,
            self.expected_owner_uid,
            BARRIER_VERSION,
            manifest_id,
        )?;
        fs::rename(&retiring, &retired).map_err(io_error)?;
        sync_dir(&self.retiring_dir())?;
        sync_dir(&self.retired_dir())?;
        validate_record(
            &retired,
            self.expected_owner_uid,
            BARRIER_VERSION,
            manifest_id,
        )
    }

    pub fn require_retired(&self, manifest_id: &str) -> Result<(), CandidateLeaseError> {
        self.prepare_layout()?;
        validate_manifest_id(manifest_id)?;
        let path = self.retired_path(&hex_text(manifest_id));
        if !path_present(&path)? {
            return Err(CandidateLeaseError::Retiring);
        }
        validate_record(
            &path,
            self.expected_owner_uid,
            BARRIER_VERSION,
            manifest_id,
        )
    }

    /// Safe only at daemon startup after systemd has destroyed the previous
    /// service control group. Durable begin jobs are recovered separately.
    pub fn recover_abandoned_enqueue_leases(&self) -> Result<usize, CandidateLeaseError> {
        self.prepare_layout()?;
        let mut removed = 0usize;
        for path in self.active_paths()? {
            let text = read_protected_text(&path, self.expected_owner_uid)?;
            let _ = parse_record(&text, LEASE_VERSION)?;
            fs::remove_file(&path).map_err(io_error)?;
            removed += 1;
        }
        if removed > 0 {
            sync_dir(&self.active_dir())?;
        }
        Ok(removed)
    }

    fn reject_if_barred(
        &self,
        key: &str,
        manifest_id: &str,
    ) -> Result<(), CandidateLeaseError> {
        let retired = self.retired_path(key);
        if path_present(&retired)? {
            validate_record(
                &retired,
                self.expected_owner_uid,
                BARRIER_VERSION,
                manifest_id,
            )?;
            return Err(CandidateLeaseError::Retired);
        }
        let retiring = self.retiring_path(key);
        if path_present(&retiring)? {
            validate_record(
                &retiring,
                self.expected_owner_uid,
                BARRIER_VERSION,
                manifest_id,
            )?;
            return Err(CandidateLeaseError::Retiring);
        }
        Ok(())
    }

    fn active_paths(&self) -> Result<Vec<PathBuf>, CandidateLeaseError> {
        let mut paths = fs::read_dir(self.active_dir())
            .map_err(io_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(io_error)?
            .into_iter()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("lease"))
            .collect::<Vec<_>>();
        paths.sort();
        Ok(paths)
    }

    fn active_dir(&self) -> PathBuf {
        self.root.join(ACTIVE)
    }

    fn retiring_dir(&self) -> PathBuf {
        self.root.join(RETIRING)
    }

    fn retired_dir(&self) -> PathBuf {
        self.root.join(RETIRED)
    }

    fn retiring_path(&self, key: &str) -> PathBuf {
        self.retiring_dir().join(format!("{key}.barrier"))
    }

    fn retired_path(&self, key: &str) -> PathBuf {
        self.retired_dir().join(format!("{key}.barrier"))
    }
}

#[derive(Debug)]
struct ParsedRecord {
    manifest_id: String,
}

fn canonical_record(version: &str, manifest_id: &str, created_at_unix_ms: u64) -> String {
    format!(
        "{version}\nmanifest-id={}\ncreated-at-unix-ms={created_at_unix_ms}\n",
        hex_text(manifest_id)
    )
}

fn parse_record(text: &str, expected_version: &str) -> Result<ParsedRecord, CandidateLeaseError> {
    let mut lines = text.split_terminator('\n');
    if lines.next() != Some(expected_version) {
        return Err(CandidateLeaseError::Malformed);
    }
    let manifest = lines
        .next()
        .and_then(|line| line.strip_prefix("manifest-id="))
        .ok_or(CandidateLeaseError::Malformed)?;
    let created = lines
        .next()
        .and_then(|line| line.strip_prefix("created-at-unix-ms="))
        .ok_or(CandidateLeaseError::Malformed)?;
    if lines.next().is_some() || !text.ends_with('\n') {
        return Err(CandidateLeaseError::Malformed);
    }
    let manifest_id = decode_hex_text(manifest)?;
    validate_manifest_id(&manifest_id)?;
    let created_at_unix_ms = created
        .parse::<u64>()
        .map_err(|_| CandidateLeaseError::Malformed)?;
    if canonical_record(expected_version, &manifest_id, created_at_unix_ms) != text {
        return Err(CandidateLeaseError::Malformed);
    }
    Ok(ParsedRecord { manifest_id })
}

fn validate_record(
    path: &Path,
    owner: u32,
    version: &str,
    manifest_id: &str,
) -> Result<(), CandidateLeaseError> {
    let text = read_protected_text(path, owner)?;
    let parsed = parse_record(&text, version)?;
    if parsed.manifest_id != manifest_id {
        return Err(CandidateLeaseError::Malformed);
    }
    Ok(())
}

fn validate_manifest_id(value: &str) -> Result<(), CandidateLeaseError> {
    if value.is_empty() || value.len() > 512 || value.contains('\n') || value.contains('\r') {
        return Err(CandidateLeaseError::InvalidManifestId);
    }
    Ok(())
}

fn create_or_validate_directory(path: &Path, owner: u32) -> Result<(), CandidateLeaseError> {
    match fs::symlink_metadata(path) {
        Ok(_) => validate_directory(path, owner),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.mode(0o700);
            match builder.create(path) {
                Ok(()) => {
                    sync_dir(path.parent().ok_or(CandidateLeaseError::InvalidLayout)?)?;
                    validate_directory(path, owner)
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    validate_directory(path, owner)
                }
                Err(error) => Err(io_error(error)),
            }
        }
        Err(error) => Err(io_error(error)),
    }
}

fn validate_directory(path: &Path, owner: u32) -> Result<(), CandidateLeaseError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(CandidateLeaseError::InvalidLayout);
    }
    Ok(())
}

fn path_present(path: &Path) -> Result<bool, CandidateLeaseError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(error)),
    }
}

fn create_protected_file(path: &Path, body: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(body.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

fn read_protected_text(path: &Path, owner: u32) -> Result<String, CandidateLeaseError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.len() > MAX_RECORD_BYTES
    {
        return Err(CandidateLeaseError::OwnerMismatch);
    }
    let mut text = String::new();
    File::open(path)
        .map_err(io_error)?
        .take(MAX_RECORD_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(io_error)?;
    if text.len() as u64 > MAX_RECORD_BYTES {
        return Err(CandidateLeaseError::Malformed);
    }
    Ok(text)
}

fn sync_dir(path: &Path) -> Result<(), CandidateLeaseError> {
    File::open(path).and_then(|file| file.sync_all()).map_err(io_error)
}

fn io_error(error: std::io::Error) -> CandidateLeaseError {
    CandidateLeaseError::Io(error.to_string())
}

fn now_unix_ms() -> Result<u64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;
    u64::try_from(duration.as_millis()).map_err(|_| "clock overflow".into())
}

fn random_hex_128() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")
        .map_err(|error| error.to_string())?
        .read_exact(&mut bytes)
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

fn decode_hex_text(value: &str) -> Result<String, CandidateLeaseError> {
    if value.len() % 2 != 0 {
        return Err(CandidateLeaseError::Malformed);
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let text = std::str::from_utf8(pair).map_err(|_| CandidateLeaseError::Malformed)?;
        let byte = u8::from_str_radix(text, 16).map_err(|_| CandidateLeaseError::Malformed)?;
        bytes.push(byte);
    }
    String::from_utf8(bytes).map_err(|_| CandidateLeaseError::Malformed)
}
