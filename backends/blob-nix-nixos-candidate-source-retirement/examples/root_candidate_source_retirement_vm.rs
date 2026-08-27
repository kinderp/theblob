#![forbid(unsafe_code)]

use std::env;
use std::process::ExitCode;

use blob_nix_nixos_candidate_source_retirement::RootCandidateSourceRetirement;

fn arg(args: &[String], flag: &str) -> Result<String, String> {
    let position = args.iter().position(|value| value == flag)
        .ok_or_else(|| format!("missing {flag}"))?;
    args.get(position + 1).cloned()
        .ok_or_else(|| format!("missing value after {flag}"))
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let manifest_id = arg(&args, "--manifest-id")?;
    let now_ms = arg(&args, "--now-ms")?.parse::<u64>()
        .map_err(|_| "invalid --now-ms".to_owned())?;
    let retention_ms = args.iter().position(|value| value == "--retention-ms")
        .and_then(|position| args.get(position + 1))
        .map(|value| value.parse::<u64>().map_err(|_| "invalid --retention-ms".to_owned()))
        .transpose()?
        .unwrap_or(0);

    let retirement = RootCandidateSourceRetirement::production_default();
    let disposition = retirement
        .retire_candidate_and_source(&manifest_id, now_ms, retention_ms)
        .map_err(|error| format!("candidate source retirement rejected: {error:?}"))?;
    println!("manifest-id={manifest_id}");
    println!("source-retirement={disposition:?}");
    println!("source-gcroot={}", retirement.source_gcroot_path(&manifest_id).display());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("BLOB_CANDIDATE_SOURCE_RETIREMENT_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
