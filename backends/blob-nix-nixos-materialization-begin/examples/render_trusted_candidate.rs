#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use blob_core::{SystemCandidateId, SystemSpecId};
use blob_nix_nixos_materialization_begin::{
    canonical_trusted_candidate, TrustedMaterializationCandidate,
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
    let manifest = TrustedMaterializationCandidate {
        manifest_id: value(&args, "--manifest-id")?,
        candidate: SystemCandidateId::from(value(&args, "--candidate")?),
        system_spec: SystemSpecId::from(value(&args, "--system-spec")?),
        immutable_flake_root: PathBuf::from(value(&args, "--source")?),
        installable_attribute: value(&args, "--attribute")?,
        provenance: vec!["fixture:root-staged-trusted-candidate".into()],
    };
    print!("{}", canonical_trusted_candidate(&manifest));
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("BLOB_TRUSTED_CANDIDATE_RENDER_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
