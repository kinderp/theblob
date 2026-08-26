#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use blob_core::{NodeId, SystemOperationId};
use blob_nix_nixos_materialization_authority::StdNixMaterializationInspector;
use blob_nix_nixos_materialization_begin::RootMaterializationBeginBoundary;

const LOCAL_NODE: &str = "node:blob-prepared-request-daemon-vm";

enum Mode {
    Begin,
    Resume,
    Complete,
}

struct Args {
    mode: Mode,
    manifest_id: Option<String>,
    operation: Option<SystemOperationId>,
    nix: PathBuf,
    nix_store: PathBuf,
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

fn optional_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|position| args.get(position + 1))
        .cloned()
}

fn parse_args() -> Result<Args, String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mode = match value(&args, "--mode")?.as_str() {
        "begin" => Mode::Begin,
        "resume" => Mode::Resume,
        "complete" => Mode::Complete,
        other => return Err(format!("unsupported mode: {other}")),
    };
    Ok(Args {
        mode,
        manifest_id: optional_value(&args, "--manifest-id"),
        operation: optional_value(&args, "--operation").map(SystemOperationId::from),
        nix: PathBuf::from(value(&args, "--nix")?),
        nix_store: PathBuf::from(value(&args, "--nix-store")?),
    })
}

fn print_intent(intent: &blob_nix_nixos_materialization_authority::MaterializationIntent) {
    println!("operation={}", intent.materialization_operation);
    println!("candidate={}", intent.candidate);
    println!("system-spec={}", intent.system_spec);
    println!("source={}", intent.immutable_flake_root.display());
    println!("attribute={}", intent.installable_attribute);
    println!("derivation={}", intent.derivation_path.display());
    println!("expected-output={}", intent.expected_output.display());
    println!("build-target={}", intent.build_target());
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let inspector = StdNixMaterializationInspector::new(args.nix, args.nix_store);
    let boundary = RootMaterializationBeginBoundary::production_default(NodeId::from(LOCAL_NODE));

    match args.mode {
        Mode::Begin => {
            let manifest_id = args
                .manifest_id
                .ok_or_else(|| "missing --manifest-id".to_owned())?;
            let intent = boundary
                .begin(&manifest_id, &inspector)
                .map_err(|error| format!("manifest-only materialization begin rejected: {error:?}"))?;
            print_intent(&intent);
        }
        Mode::Resume => {
            let operation = args
                .operation
                .ok_or_else(|| "missing --operation".to_owned())?;
            let intent = boundary
                .resume(&operation)
                .map_err(|error| format!("materialization resume rejected: {error:?}"))?;
            print_intent(&intent);
        }
        Mode::Complete => {
            let operation = args
                .operation
                .ok_or_else(|| "missing --operation".to_owned())?;
            let admission = boundary
                .complete(&operation, &inspector)
                .map_err(|error| format!("materialization complete rejected: {error:?}"))?;
            println!("operation={}", admission.materialization_operation);
            println!("candidate={}", admission.candidate);
            println!("system-spec={}", admission.system_spec);
            println!("closure={}", admission.system_closure);
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("BLOB_MATERIALIZATION_BEGIN_BOUNDARY_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
