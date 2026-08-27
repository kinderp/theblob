#![forbid(unsafe_code)]

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use blob_core::SystemOperationId;

pub const DEFAULT_CANDIDATE_ENQUEUE_LEASE_ROOT: &str =
    "/var/lib/theblob/materialization-candidate-enqueue-leases";
pub const MAX_CANDIDATE_ENQUEUE_LEASE_BYTES: u64 = 16 * 1024;
pub const MAX_CANDIDATE_ENQUEUE_LEASES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateEnqueueLease {
    pub manifest_id: String,
    pub request_id: String,
    pub operation: SystemOperationId,
    pub requester_uid: u32,
    pub requester_system_bus_name: String,
    pub acquired_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateEnqueueLeaseError {
    InvalidLayout,
    InvalidIdentifier,
    InvalidFile,
    TooLarge,
    Malformed,
    NonCanonical,
    CapacityExceeded,
    Missing(String),
    Conflict,
    Io(String),
}

pub struct FileCandidateEnqueueLeaseStore {
    root: PathBuf,
    expected_owner_uid: u32,
}

impl FileCandidateEnqueueLeaseStore {
    pub fn production_default() -> Self {
        Self::new(DEFAULT_CANDIDATE_ENQUEUE_LEASE_ROOT, 0)
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

    /// Ensure the transient lease directory exists only beneath an already
    /// protected parent. This is used by enqueue/startup so introducing leases
    /// does not require every older VM/service definition to pre-create the new
    /// child directory. Reclamation code should call `validate_layout` instead:
    /// a missing lease root must never be interpreted as "no leases".
    pub fn ensure_layout(&self) -> Result<(), CandidateEnqueueLeaseError> {
        match fs::symlink_metadata(&self.root) {
            Ok(_) => self.validate_layout(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let parent = self
                    .root
                    .parent()
                    .ok_or(CandidateEnqueueLeaseError::InvalidLayout)?;
                validate_protected_directory(parent, self.expected_owner_uid)?;
                let mut builder = DirBuilder::new();
                builder.mode(0o700);
                match builder.create(&self.root) {
                    Ok(()) => {
                        sync_dir(parent)?;
                        self.validate_layout()
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        self.validate_layout()
                    }
                    Err(error) => Err(CandidateEnqueueLeaseError::Io(error.to_string())),
                }
            }
            Err(error) => Err(CandidateEnqueueLeaseError::Io(error.to_string())),
        }
    }

    pub fn acquire(
        &self,
        lease: &CandidateEnqueueLease,
    ) -> Result<(), CandidateEnqueueLeaseError> {
        self.ensure_layout()?;
        validate_lease(lease)?;
        if self.list()?.len() >= MAX_CANDIDATE_ENQUEUE_LEASES {
            return Err(CandidateEnqueueLeaseError::CapacityExceeded);
        }
        let path = self.path(lease);
        let text = canonical_candidate_enqueue_lease(lease);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    CandidateEnqueueLeaseError::Conflict
                } else {
                    CandidateEnqueueLeaseError::Io(error.to_string())
                }
            })?;
        file.write_all(text.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|error| CandidateEnqueueLeaseError::Io(error.to_string()))?;
        sync_dir(&self.root)
    }

    pub fn release(
        &self,
        lease: &CandidateEnqueueLease,
    ) -> Result<(), CandidateEnqueueLeaseError> {
        self.validate_layout()?;
        let path = self.path(lease);
        let observed = self.load_path(&path)?;
        if observed != *lease {
            return Err(CandidateEnqueueLeaseError::Conflict);
        }
        fs::remove_file(&path)
            .map_err(|error| CandidateEnqueueLeaseError::Io(error.to_string()))?;
        sync_dir(&self.root)
    }

    pub fn list(&self) -> Result<Vec<CandidateEnqueueLease>, CandidateEnqueueLeaseError> {
        self.validate_layout()?;
        let mut paths = fs::read_dir(&self.root)
            .map_err(|error| CandidateEnqueueLeaseError::Io(error.to_string()))?
            .map(|entry| {
                entry
                    .map(|value| value.path())
                    .map_err(|error| CandidateEnqueueLeaseError::Io(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        if paths.len() > MAX_CANDIDATE_ENQUEUE_LEASES {
            return Err(CandidateEnqueueLeaseError::CapacityExceeded);
        }
        let mut leases = Vec::with_capacity(paths.len());
        for path in paths {
            if path.extension().and_then(|value| value.to_str()) != Some("lease") {
                return Err(CandidateEnqueueLeaseError::InvalidFile);
            }
            leases.push(self.load_path(&path)?);
        }
        Ok(leases)
    }

    pub fn for_manifest(
        &self,
        manifest_id: &str,
    ) -> Result<Vec<CandidateEnqueueLease>, CandidateEnqueueLeaseError> {
        validate_id(manifest_id, "manifest:")?;
        Ok(self
            .list()?
            .into_iter()
            .filter(|lease| lease.manifest_id == manifest_id)
            .collect())
    }

    pub fn path(&self, lease: &CandidateEnqueueLease) -> PathBuf {
        self.root.join(format!(
            "manifest-{}-request-{}.lease",
            hex_text(&lease.manifest_id),
            hex_text(&lease.request_id)
        ))
    }

    pub fn validate_layout(&self) -> Result<(), CandidateEnqueueLeaseError> {
        validate_protected_directory(&self.root, self.expected_owner_uid)
    }

    fn load_path(&self, path: &Path) -> Result<CandidateEnqueueLease, CandidateEnqueueLeaseError> {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                CandidateEnqueueLeaseError::Missing(path.display().to_string())
            } else {
                CandidateEnqueueLeaseError::Io(error.to_string())
            }
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != self.expected_owner_uid
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(CandidateEnqueueLeaseError::InvalidFile);
        }
        if metadata.len() > MAX_CANDIDATE_ENQUEUE_LEASE_BYTES {
            return Err(CandidateEnqueueLeaseError::TooLarge);
        }
        let mut text = String::new();
        File::open(path)
            .and_then(|mut file| file.read_to_string(&mut text))
            .map_err(|error| CandidateEnqueueLeaseError::Io(error.to_string()))?;
        let lease = parse_candidate_enqueue_lease(&text)?;
        if canonical_candidate_enqueue_lease(&lease) != text || self.path(&lease) != path {
            return Err(CandidateEnqueueLeaseError::NonCanonical);
        }
        Ok(lease)
    }
}

pub fn canonical_candidate_enqueue_lease(lease: &CandidateEnqueueLease) -> String {
    [
        "theblob-candidate-enqueue-lease-v1".to_owned(),
        format!("manifest-id={}", hex_text(&lease.manifest_id)),
        format!("request-id={}", hex_text(&lease.request_id)),
        format!("operation={}", hex_text(lease.operation.as_str())),
        format!("requester-uid={}", lease.requester_uid),
        format!(
            "requester-system-bus={}",
            hex_text(&lease.requester_system_bus_name)
        ),
        format!("acquired-at-unix-ms={}", lease.acquired_at_unix_ms),
        String::new(),
    ]
    .join("\n")
}

pub fn parse_candidate_enqueue_lease(
    text: &str,
) -> Result<CandidateEnqueueLease, CandidateEnqueueLeaseError> {
    if text.len() as u64 > MAX_CANDIDATE_ENQUEUE_LEASE_BYTES {
        return Err(CandidateEnqueueLeaseError::TooLarge);
    }
    let mut lines = text.split('\n');
    if lines.next() != Some("theblob-candidate-enqueue-lease-v1") {
        return Err(CandidateEnqueueLeaseError::Malformed);
    }
    let manifest_id = decode_field(lines.next(), "manifest-id")?;
    let request_id = decode_field(lines.next(), "request-id")?;
    let operation = SystemOperationId::from(decode_field(lines.next(), "operation")?);
    let requester_uid = plain_field(lines.next(), "requester-uid")?
        .parse::<u32>()
        .map_err(|_| CandidateEnqueueLeaseError::Malformed)?;
    let requester_system_bus_name = decode_field(lines.next(), "requester-system-bus")?;
    let acquired_at_unix_ms = plain_field(lines.next(), "acquired-at-unix-ms")?
        .parse::<u64>()
        .map_err(|_| CandidateEnqueueLeaseError::Malformed)?;
    if lines.next() != Some("") || lines.next().is_some() {
        return Err(CandidateEnqueueLeaseError::Malformed);
    }
    let lease = CandidateEnqueueLease {
        manifest_id,
        request_id,
        operation,
        requester_uid,
        requester_system_bus_name,
        acquired_at_unix_ms,
    };
    validate_lease(&lease)?;
    Ok(lease)
}

fn validate_lease(lease: &CandidateEnqueueLease) -> Result<(), CandidateEnqueueLeaseError> {
    validate_id(&lease.manifest_id, "manifest:")?;
    validate_id(&lease.request_id, "begin-request:")?;
    validate_id(lease.operation.as_str(), "op:materialize-")?;
    if !lease.requester_system_bus_name.starts_with(':')
        || lease.requester_system_bus_name.len() > 128
        || !lease
            .requester_system_bus_name
            .bytes()
            .all(|byte| byte.is_ascii_graphic())
    {
        return Err(CandidateEnqueueLeaseError::InvalidIdentifier);
    }
    Ok(())
}

fn validate_id(value: &str, prefix: &str) -> Result<(), CandidateEnqueueLeaseError> {
    if !value.starts_with(prefix)
        || value.len() > 256
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(CandidateEnqueueLeaseError::InvalidIdentifier);
    }
    Ok(())
}

fn plain_field<'a>(
    line: Option<&'a str>,
    key: &str,
) -> Result<&'a str, CandidateEnqueueLeaseError> {
    line.and_then(|value| value.strip_prefix(&format!("{key}=")))
        .ok_or(CandidateEnqueueLeaseError::Malformed)
}

fn decode_field(
    line: Option<&str>,
    key: &str,
) -> Result<String, CandidateEnqueueLeaseError> {
    decode_hex(plain_field(line, key)?)
}

fn decode_hex(value: &str) -> Result<String, CandidateEnqueueLeaseError> {
    if value.len() % 2 != 0 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CandidateEnqueueLeaseError::Malformed);
    }
    let bytes = value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).map_err(|_| CandidateEnqueueLeaseError::Malformed)?;
            u8::from_str_radix(text, 16).map_err(|_| CandidateEnqueueLeaseError::Malformed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    String::from_utf8(bytes).map_err(|_| CandidateEnqueueLeaseError::Malformed)
}

fn validate_protected_directory(
    path: &Path,
    expected_owner_uid: u32,
) -> Result<(), CandidateEnqueueLeaseError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| CandidateEnqueueLeaseError::InvalidLayout)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != expected_owner_uid
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(CandidateEnqueueLeaseError::InvalidLayout);
    }
    Ok(())
}

fn hex_text(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sync_dir(path: &Path) -> Result<(), CandidateEnqueueLeaseError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| CandidateEnqueueLeaseError::Io(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lease_round_trip_is_canonical() {
        let lease = CandidateEnqueueLease {
            manifest_id: "manifest:test".into(),
            request_id: "begin-request:abc".into(),
            operation: SystemOperationId::from("op:materialize-def"),
            requester_uid: 1000,
            requester_system_bus_name: ":1.44".into(),
            acquired_at_unix_ms: 42,
        };
        let text = canonical_candidate_enqueue_lease(&lease);
        assert_eq!(parse_candidate_enqueue_lease(&text).unwrap(), lease);
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn parser_rejects_trailing_fields() {
        let lease = CandidateEnqueueLease {
            manifest_id: "manifest:test".into(),
            request_id: "begin-request:abc".into(),
            operation: SystemOperationId::from("op:materialize-def"),
            requester_uid: 1000,
            requester_system_bus_name: ":1.44".into(),
            acquired_at_unix_ms: 42,
        };
        let text = canonical_candidate_enqueue_lease(&lease) + "extra=1\n";
        assert_eq!(
            parse_candidate_enqueue_lease(&text),
            Err(CandidateEnqueueLeaseError::Malformed)
        );
    }
}
