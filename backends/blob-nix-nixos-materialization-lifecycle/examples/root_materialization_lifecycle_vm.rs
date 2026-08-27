#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use blob_core::{NodeId, SystemOperationId};
use blob_nix_nixos_materialization_authority::StdNixMaterializationInspector;
use blob_nix_nixos_materialization_lifecycle::{
    RootMaterializationLifecycleManager, RootSafeMaterializationFinalizer,
};

const LOCAL_NODE: &str = "node:blob-prepared-request-daemon-vm";

enum Mode {
    Finalize,
    Cancel,
    Expire,
    ReconcileDerivation,
    RetireCandidate,
    RetireJob,
}

struct Args {
    mode: Mode,
    operation: Option<SystemOperationId>,
    request_id: Option<String>,
    manifest_id: Option<String>,
    uid: Option<u32>,
    now_ms: Option<u64>,
    retention_ms: u64,
    nix: Option<PathBuf>,
    nix_store: Option<PathBuf>,
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
        "finalize" => Mode::Finalize,
        "cancel" => Mode::Cancel,
        "expire" => Mode::Expire,
        "reconcile-derivation" => Mode::ReconcileDerivation,
        "retire-candidate" => Mode::RetireCandidate,
        "retire-job" => Mode::RetireJob,
        other => return Err(format!("unsupported mode: {other}")),
    };
    Ok(Args {
        mode,
        operation: optional_value(&args, "--operation").map(SystemOperationId::from),
        request_id: optional_value(&args, "--request-id"),
        manifest_id: optional_value(&args, "--manifest-id"),
        uid: optional_value(&args, "--uid")
            .map(|value| value.parse::<u32>().map_err(|_| "invalid --uid".to_owned()))
            .transpose()?,
        now_ms: optional_value(&args, "--now-ms")
            .map(|value| value.parse::<u64>().map_err(|_| "invalid --now-ms".to_owned()))
            .transpose()?,
        retention_ms: optional_value(&args, "--retention-ms")
            .map(|value| value.parse::<u64>().map_err(|_| "invalid --retention-ms".to_owned()))
            .transpose()?
            .unwrap_or(0),
        nix: optional_value(&args, "--nix").map(PathBuf::from),
        nix_store: optional_value(&args, "--nix-store").map(PathBuf::from),
    })
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    match args.mode {
        Mode::Finalize => {
            let operation = args
                .operation
                .ok_or_else(|| "missing --operation".to_owned())?;
            let nix = args.nix.ok_or_else(|| "missing --nix".to_owned())?;
            let nix_store = args
                .nix_store
                .ok_or_else(|| "missing --nix-store".to_owned())?;
            let inspector = StdNixMaterializationInspector::new(nix, nix_store);
            let finalizer =
                RootSafeMaterializationFinalizer::production_default(NodeId::from(LOCAL_NODE));
            let admission = finalizer
                .complete(&operation, &inspector)
                .map_err(|error| format!("safe finalization rejected: {error:?}"))?;
            println!("operation={}", admission.materialization_operation);
            println!("candidate={}", admission.candidate);
            println!("system-spec={}", admission.system_spec);
            println!("closure={}", admission.system_closure);
            println!(
                "closure-gcroot={}",
                finalizer.admitted_closure_gcroot_path(&operation).display()
            );
        }
        Mode::Cancel => {
            let request_id = args
                .request_id
                .ok_or_else(|| "missing --request-id".to_owned())?;
            let uid = args.uid.ok_or_else(|| "missing --uid".to_owned())?;
            let now_ms = args.now_ms.ok_or_else(|| "missing --now-ms".to_owned())?;
            RootMaterializationLifecycleManager::production_default()
                .cancel_queued(&request_id, uid, now_ms)
                .map_err(|error| format!("queued cancellation rejected: {error:?}"))?;
            println!("cancelled-request={request_id}");
        }
        Mode::Expire => {
            let request_id = args
                .request_id
                .ok_or_else(|| "missing --request-id".to_owned())?;
            let now_ms = args.now_ms.ok_or_else(|| "missing --now-ms".to_owned())?;
            RootMaterializationLifecycleManager::production_default()
                .expire_queued(&request_id, now_ms, args.retention_ms)
                .map_err(|error| format!("queued expiry rejected: {error:?}"))?;
            println!("expired-request={request_id}");
        }
        Mode::ReconcileDerivation => {
            let operation = args
                .operation
                .ok_or_else(|| "missing --operation".to_owned())?;
            let now_ms = args.now_ms.ok_or_else(|| "missing --now-ms".to_owned())?;
            let disposition = RootMaterializationLifecycleManager::production_default()
                .reconcile_derivation_gcroot(&operation, now_ms)
                .map_err(|error| format!("derivation reconcile rejected: {error:?}"))?;
            println!("derivation-disposition={disposition:?}");
        }
        Mode::RetireCandidate => {
            let manifest_id = args
                .manifest_id
                .ok_or_else(|| "missing --manifest-id".to_owned())?;
            let now_ms = args.now_ms.ok_or_else(|| "missing --now-ms".to_owned())?;
            RootMaterializationLifecycleManager::production_default()
                .retire_candidate(&manifest_id, now_ms, args.retention_ms)
                .map_err(|error| format!("candidate retirement rejected: {error:?}"))?;
            println!("retired-manifest={manifest_id}");
        }
        Mode::RetireJob => {
            let request_id = args
                .request_id
                .ok_or_else(|| "missing --request-id".to_owned())?;
            let now_ms = args.now_ms.ok_or_else(|| "missing --now-ms".to_owned())?;
            RootMaterializationLifecycleManager::production_default()
                .retire_terminal_job(&request_id, now_ms, args.retention_ms)
                .map_err(|error| format!("terminal job retirement rejected: {error:?}"))?;
            println!("retired-request={request_id}");
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("BLOB_MATERIALIZATION_LIFECYCLE_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
