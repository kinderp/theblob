#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use blob_core::NodeId;
use blob_nix_nixos_materialization_authority::StdNixMaterializationInspector;
use blob_nix_nixos_materialization_begin_queue::{
    FileMaterializationBeginQueue, MaterializationBeginJobState,
    RecoverableMaterializationBeginCoordinator,
};

const LOCAL_NODE: &str = "node:blob-prepared-request-daemon-vm";

enum Mode {
    Enqueue,
    Recover,
    WorkOne,
    Status,
}

struct Args {
    mode: Mode,
    sender: Option<String>,
    uid: Option<u32>,
    manifest_id: Option<String>,
    request_id: Option<String>,
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
        "enqueue" => Mode::Enqueue,
        "recover" => Mode::Recover,
        "work-one" => Mode::WorkOne,
        "status" => Mode::Status,
        other => return Err(format!("unsupported mode: {other}")),
    };
    let uid = optional_value(&args, "--uid")
        .map(|value| value.parse::<u32>().map_err(|_| "invalid --uid".to_owned()))
        .transpose()?;
    Ok(Args {
        mode,
        sender: optional_value(&args, "--sender"),
        uid,
        manifest_id: optional_value(&args, "--manifest-id"),
        request_id: optional_value(&args, "--request-id"),
        nix: optional_value(&args, "--nix").map(PathBuf::from),
        nix_store: optional_value(&args, "--nix-store").map(PathBuf::from),
    })
}

fn state_name(state: MaterializationBeginJobState) -> &'static str {
    match state {
        MaterializationBeginJobState::Queued => "queued",
        MaterializationBeginJobState::Running => "running",
        MaterializationBeginJobState::Completed => "completed",
        MaterializationBeginJobState::Failed => "failed",
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let queue = FileMaterializationBeginQueue::production_default();

    match args.mode {
        Mode::Enqueue => {
            let sender = args.sender.ok_or_else(|| "missing --sender".to_owned())?;
            let uid = args.uid.ok_or_else(|| "missing --uid".to_owned())?;
            let manifest_id = args
                .manifest_id
                .ok_or_else(|| "missing --manifest-id".to_owned())?;
            let job = queue
                .enqueue(uid, &sender, &manifest_id)
                .map_err(|error| format!("enqueue rejected: {error:?}"))?;
            println!("request-id={}", job.request_id);
            println!("operation={}", job.operation);
            println!("manifest-id={}", job.manifest_id);
            println!("requester-uid={}", job.requester_uid);
            println!("requester-system-bus={}", job.requester_system_bus_name);
        }
        Mode::Recover => {
            let recovered = queue
                .recover_running()
                .map_err(|error| format!("recovery rejected: {error:?}"))?;
            println!("recovered={recovered}");
        }
        Mode::WorkOne => {
            let Some(job) = queue
                .claim_next()
                .map_err(|error| format!("claim rejected: {error:?}"))?
            else {
                println!("none=true");
                return Ok(());
            };

            println!("request-id={}", job.request_id);
            println!("operation={}", job.operation);
            println!("state=running");

            let nix = args.nix.ok_or_else(|| "missing --nix".to_owned())?;
            let nix_store = args
                .nix_store
                .ok_or_else(|| "missing --nix-store".to_owned())?;
            let inspector = StdNixMaterializationInspector::new(nix, nix_store);
            let coordinator = RecoverableMaterializationBeginCoordinator::production_default(
                NodeId::from(LOCAL_NODE),
            );

            match coordinator.start_or_reconcile(&job, &inspector) {
                Ok(intent) => {
                    queue
                        .mark_completed(&job.request_id)
                        .map_err(|error| format!("completion state rejected: {error:?}"))?;
                    println!("state=completed");
                    println!("candidate={}", intent.candidate);
                    println!("system-spec={}", intent.system_spec);
                    println!("source={}", intent.immutable_flake_root.display());
                    println!("attribute={}", intent.installable_attribute);
                    println!("derivation={}", intent.derivation_path.display());
                    println!("expected-output={}", intent.expected_output.display());
                    println!("build-target={}", intent.build_target());
                }
                Err(error) => {
                    let mark = queue.mark_failed(&job.request_id);
                    if let Err(mark_error) = mark {
                        return Err(format!(
                            "worker failed with {error:?}; marking failed also rejected: {mark_error:?}"
                        ));
                    }
                    return Err(format!("worker failed: {error:?}"));
                }
            }
        }
        Mode::Status => {
            let request_id = args
                .request_id
                .ok_or_else(|| "missing --request-id".to_owned())?;
            let uid = args.uid.ok_or_else(|| "missing --uid".to_owned())?;
            let status = queue
                .status_for_uid(&request_id, uid)
                .map_err(|error| format!("status rejected: {error:?}"))?;
            println!("request-id={}", status.job.request_id);
            println!("state={}", state_name(status.state));
            println!("operation={}", status.job.operation);
            println!("manifest-id={}", status.job.manifest_id);
            println!("requester-uid={}", status.job.requester_uid);
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("BLOB_ASYNC_MATERIALIZATION_BEGIN_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
