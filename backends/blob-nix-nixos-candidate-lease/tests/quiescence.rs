#![cfg(unix)]

use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use blob_nix_nixos_candidate_lease::{CandidateEnqueueLeaseManager, CandidateLeaseError};

fn sandbox(label: &str) -> (PathBuf, CandidateEnqueueLeaseManager) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let parent = std::env::temp_dir().join(format!("theblob-{label}-{nonce}"));
    let mut builder = DirBuilder::new();
    builder.mode(0o700);
    builder.create(&parent).unwrap();
    let uid = fs::symlink_metadata(&parent).unwrap().uid();
    let manager = CandidateEnqueueLeaseManager::new(parent.join("leases"), uid);
    (parent, manager)
}

#[test]
fn active_enqueue_blocks_retirement_until_lease_is_released() {
    let (parent, manager) = sandbox("lease-busy");
    let manifest = "manifest:systemspec-test";

    let lease = manager.acquire_enqueue(manifest).unwrap();
    manager.begin_retirement(manifest).unwrap();
    assert!(matches!(
        manager.require_quiescent(manifest),
        Err(CandidateLeaseError::Busy)
    ));

    lease.release().unwrap();
    manager.require_quiescent(manifest).unwrap();
    manager.mark_retired(manifest).unwrap();
    manager.require_retired(manifest).unwrap();
    assert!(matches!(
        manager.acquire_enqueue(manifest),
        Err(CandidateLeaseError::Retired)
    ));

    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn retirement_barrier_prevents_new_enqueue_before_candidate_access() {
    let (parent, manager) = sandbox("barrier-first");
    let manifest = "manifest:systemspec-test";

    manager.begin_retirement(manifest).unwrap();
    assert!(matches!(
        manager.acquire_enqueue(manifest),
        Err(CandidateLeaseError::Retiring)
    ));
    manager.require_quiescent(manifest).unwrap();
    manager.mark_retired(manifest).unwrap();

    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn startup_recovery_clears_only_abandoned_active_leases() {
    let (parent, manager) = sandbox("lease-recovery");
    let manifest = "manifest:systemspec-test";

    let lease = manager.acquire_enqueue(manifest).unwrap();
    std::mem::forget(lease);
    assert_eq!(manager.recover_abandoned_enqueue_leases().unwrap(), 1);
    assert_eq!(manager.recover_abandoned_enqueue_leases().unwrap(), 0);

    fs::remove_dir_all(parent).unwrap();
}
