#![forbid(unsafe_code)]

use std::env;
use std::process::ExitCode;

use blob_core::{
    NodeId, SystemAuthorityClass, SystemAuthorizationId, SystemCandidateAction, SystemCandidateId,
    SystemEffectClass, SystemOperationId, SystemSpecId,
};
use blob_nix_nixos_activation::ImmutableNixOsActivationPlan;
use blob_nix_nixos_request_store::canonical_text;
use blob_system_activation_gate::PreparedPrivilegedActivation;

const LOCAL_NODE: &str = "node:blob-prepared-request-daemon-vm";

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
    let authorization = value(&args, "--authorization")?;
    let action = value(&args, "--action")?;
    let candidate = value(&args, "--candidate")?;
    let now = value(&args, "--now-ms")?
        .parse::<u64>()
        .map_err(|_| "invalid --now-ms".to_owned())?;

    let (system_action, effect_class, argument) = match action.as_str() {
        "preview" => (
            SystemCandidateAction::PreviewActivation,
            SystemEffectClass::PreviewHooks,
            "dry-activate",
        ),
        "test" => (
            SystemCandidateAction::TestActivation,
            SystemEffectClass::TemporaryLiveActivation,
            "test",
        ),
        _ => return Err(format!("unsupported action: {action}")),
    };

    if !candidate.starts_with("/nix/store/") {
        return Err("candidate is not an immutable store path".into());
    }

    let node = NodeId::from(LOCAL_NODE);
    let authorization = SystemAuthorizationId::from(authorization);
    let expires_at = now.saturating_add(300_000);
    let prepared = PreparedPrivilegedActivation {
        node: node.clone(),
        readiness_observed_at_unix_ms: now,
        authorization: authorization.clone(),
        authorization_expires_at_unix_ms: expires_at,
        prepared_at_unix_ms: now,
        plan: ImmutableNixOsActivationPlan {
            operation_id: SystemOperationId::from(format!("op:prepared-request-{action}")),
            candidate: SystemCandidateId::from("candidate:blob-prepared-request-daemon-vm"),
            system_spec: SystemSpecId::from("system:blob-prepared-request-daemon-vm"),
            materialization_operation: SystemOperationId::from(
                "op:blob-prepared-request-daemon-vm-materialize",
            ),
            system_closure: candidate.clone(),
            action: system_action,
            effect_class,
            authority: SystemAuthorityClass::HostAdministrator,
            program: format!("{candidate}/bin/switch-to-configuration"),
            args: vec![argument.into()],
            expected_effects: vec![],
            rollback_semantics: "temporary activation; reboot restores boot-default closure".into(),
        },
        readiness_evidence: vec![
            format!("node:{node}"),
            format!("observed-at-unix-ms:{now}"),
        ],
        authorization_evidence: vec![
            format!("authorization:{authorization}"),
            format!("expires-at-unix-ms:{expires_at}"),
        ],
    };

    print!("{}", canonical_text(&prepared));
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("BLOB_PREPARED_REQUEST_FIXTURE_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
