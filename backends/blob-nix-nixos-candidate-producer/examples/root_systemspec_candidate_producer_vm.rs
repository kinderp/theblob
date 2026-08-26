#![forbid(unsafe_code)]

use std::env;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use blob_nix_nixos_candidate_producer::{
    RootSystemSpecCandidateProducer, StdNixCandidateSourceBuilder,
};

struct Args {
    sender: String,
    nix: PathBuf,
    nixpkgs_source: PathBuf,
    base_module: PathBuf,
    staging_root: PathBuf,
}

fn value(args: &[String], flag: &str) -> Result<String, String> {
    let position = args
        .iter()
        .position(|arg| arg == flag)
        .ok_or_else(|| format!("missing {flag}"))?;
    args.get(position + 1)
        .cloned()
        .ok_or_else(|| format!("missing value after {flag}"))
}

fn parse_args() -> Result<Args, String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    Ok(Args {
        sender: value(&args, "--sender")?,
        nix: PathBuf::from(value(&args, "--nix")?),
        nixpkgs_source: PathBuf::from(value(&args, "--nixpkgs-source")?),
        base_module: PathBuf::from(value(&args, "--base-module")?),
        staging_root: PathBuf::from(value(&args, "--staging-root")?),
    })
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let mut canonical_spec = String::new();
    io::stdin()
        .read_to_string(&mut canonical_spec)
        .map_err(|error| format!("failed to read canonical SystemSpec: {error}"))?;

    let source_builder = StdNixCandidateSourceBuilder::new(
        args.nix,
        args.nixpkgs_source,
        args.base_module,
        args.staging_root,
        0,
    );
    let producer = RootSystemSpecCandidateProducer::production_default();
    let produced = producer
        .produce(&args.sender, &canonical_spec, &source_builder)
        .map_err(|error| format!("candidate producer rejected: {error:?}"))?;

    println!("manifest-id={}", produced.manifest.manifest_id);
    println!("candidate={}", produced.manifest.candidate);
    println!("system-spec={}", produced.manifest.system_spec);
    println!("source={}", produced.manifest.immutable_flake_root.display());
    println!("installable={}", produced.manifest.installable_attribute);
    println!("manifest={}", produced.manifest_path.display());
    println!("receipt={}", produced.receipt_path.display());
    println!("source-gcroot={}", produced.source_gcroot.display());
    println!("causal-id={}", produced.receipt.causal_id);
    for evidence in produced.receipt.translation_evidence {
        println!("translation={evidence}");
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("BLOB_SYSTEMSPEC_CANDIDATE_PRODUCER_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
