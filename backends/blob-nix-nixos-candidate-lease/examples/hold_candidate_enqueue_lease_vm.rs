#![forbid(unsafe_code)]

use std::env;
use std::io::{self, Read};
use std::process::ExitCode;

use blob_nix_nixos_candidate_lease::CandidateEnqueueLeaseManager;

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let position = args
        .iter()
        .position(|arg| arg == "--manifest-id")
        .ok_or_else(|| "missing --manifest-id".to_owned())?;
    let manifest_id = args
        .get(position + 1)
        .ok_or_else(|| "missing value after --manifest-id".to_owned())?;

    let lease = CandidateEnqueueLeaseManager::production_default()
        .acquire_enqueue(manifest_id)
        .map_err(|error| format!("lease acquire rejected: {error:?}"))?;
    println!("lease-active={manifest_id}");

    // A normal EOF releases the lease. The KVM proof SIGKILLs this process so
    // Drop cannot run, deliberately leaving a durable abandoned lease.
    let mut sink = Vec::new();
    io::stdin()
        .read_to_end(&mut sink)
        .map_err(|error| error.to_string())?;
    lease.release()
        .map_err(|error| format!("lease release rejected: {error:?}"))?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("BLOB_CANDIDATE_LEASE_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
