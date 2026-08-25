#![forbid(unsafe_code)]

use std::env;
use std::process::ExitCode;

use blob_core::{NodeId, SystemCandidateId, SystemOperationId, SystemSpecId};
use blob_nix_nixos_request_publisher::{
    canonical_materialization_admission, MaterializationAdmission,
};

fn value(args: &[String], flag: &str) -> Result<String, String> {
    let position = args
        .iter()
        .position(|arg| arg == flag)
        .ok_or_else(|| format!("missing {flag}"))?;
    args.get(position + 1)
        .cloned()
        .ok_or_else(|| format!("missing value after {flag}"))
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let admission = MaterializationAdmission {
        materialization_operation: SystemOperationId::from(value(
            &args,
            "--materialization-operation",
        )?),
        node: NodeId::from(value(&args, "--node")?),
        candidate: SystemCandidateId::from(value(&args, "--candidate")?),
        system_spec: SystemSpecId::from(value(&args, "--system-spec")?),
        system_closure: value(&args, "--system-closure")?,
        admitted_at_unix_ms: value(&args, "--admitted-at-ms")?
            .parse::<u64>()
            .map_err(|_| "invalid --admitted-at-ms".to_owned())?,
        provenance: vec!["test-fixture:root-owned-materialization-admission".into()],
    };
    print!("{}", canonical_materialization_admission(&admission));
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("BLOB_MATERIALIZATION_ADMISSION_FIXTURE_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
