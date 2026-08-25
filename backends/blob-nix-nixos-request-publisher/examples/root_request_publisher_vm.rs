#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use blob_core::{
    NodeId, PhysicalTestNodeProfile, SystemArchitecture, SystemCandidateAction, SystemOperationId,
};
use blob_nix_nixos_request_publisher::RootPreparedActivationPublisher;
use blob_node_probe::NodeSafetyConfirmations;

const LOCAL_NODE: &str = "node:blob-root-request-publisher-vm";

struct Args {
    sender: String,
    materialization_operation: SystemOperationId,
    action: SystemCandidateAction,
    pkcheck: PathBuf,
}

fn parse_args() -> Result<Args, String> {
    let args = env::args().skip(1).collect::<Vec<_>>();

    fn value(args: &[String], flag: &str) -> Result<String, String> {
        let position = args
            .iter()
            .position(|arg| arg == flag)
            .ok_or_else(|| format!("missing {flag}"))?;
        args.get(position + 1)
            .cloned()
            .ok_or_else(|| format!("missing value after {flag}"))
    }

    let action = match value(&args, "--action")?.as_str() {
        "preview" => SystemCandidateAction::PreviewActivation,
        "test" => SystemCandidateAction::TestActivation,
        other => return Err(format!("unsupported action: {other}")),
    };

    Ok(Args {
        sender: value(&args, "--sender")?,
        materialization_operation: SystemOperationId::from(value(
            &args,
            "--materialization-operation",
        )?),
        action,
        pkcheck: PathBuf::from(value(&args, "--pkcheck")?),
    })
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let node = NodeId::from(LOCAL_NODE);
    let mut profile = PhysicalTestNodeProfile::nixos_pilot(node.clone(), SystemArchitecture::X86_64);
    // Keep the production safety policy in the VM. The test disk is explicitly
    // sized above this threshold rather than weakening the readiness gate.
    profile.minimum_free_space_bytes = 8 * 1024 * 1024 * 1024;
    let confirmations = NodeSafetyConfirmations {
        enrolled: true,
        trusted: true,
        storage_health_ok: true,
        local_console_recovery_confirmed: true,
        external_power_override: None,
    };

    let publisher = RootPreparedActivationPublisher::new(
        node,
        profile,
        confirmations,
        "/var/lib/theblob/materialization-admissions",
        "/var/lib/theblob/prepared-activations",
        0,
        120_000,
    );
    let published = publisher
        .publish_user_initiated(
            args.sender,
            &args.materialization_operation,
            args.action,
            args.pkcheck,
        )
        .map_err(|error| format!("root request publisher rejected: {error:?}"))?;

    println!("authorization={}", published.authorization);
    println!("sender={}", published.authorized_system_bus_name);
    println!("request={}", published.request_path.display());
    println!("candidate={}", published.prepared.plan.candidate);
    println!("system-spec={}", published.prepared.plan.system_spec);
    println!(
        "materialization-operation={}",
        published.prepared.plan.materialization_operation
    );
    println!("closure={}", published.prepared.plan.system_closure);
    println!("action={:?}", published.prepared.plan.action);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("BLOB_ROOT_REQUEST_PUBLISHER_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
