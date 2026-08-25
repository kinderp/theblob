#![forbid(unsafe_code)]

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use blob_core::{
    NodeId, SystemAuthorityClass, SystemAuthorizationId, SystemCandidateAction,
    SystemCandidateId, SystemEffectClass, SystemOperationId, SystemSpecId,
};
use blob_nix_nixos_activation::ImmutableNixOsActivationPlan;
use blob_nix_nixos_authority::{
    PkcheckAuthorizationChecker, PolkitAuthorizationRequest, RootOwnedActivationPermitIssuer,
    StdPkcheckCommandRunner,
};
use blob_nix_nixos_privileged_helper::{
    FilePrivilegedExecutionLedger, LocalNixOsActivationHost, PrivilegedCommandOutcome,
    PrivilegedCommandRunner, StdPrivilegedCommandRunner,
};
use blob_nix_nixos_root_boundary::{
    FileTrustedActivationPermitStore, RootOwnedNixOsActivationBoundary,
};
use blob_system_activation_gate::PreparedPrivilegedActivation;

const LOCAL_NODE: &str = "node:blob-authorized-activation-vm";

#[derive(Clone, Copy, Debug)]
enum RequestedAction {
    Preview,
    Test,
}

impl RequestedAction {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "preview" => Ok(Self::Preview),
            "test" => Ok(Self::Test),
            _ => Err(format!("unsupported action: {value}")),
        }
    }

    fn system_action(self) -> SystemCandidateAction {
        match self {
            Self::Preview => SystemCandidateAction::PreviewActivation,
            Self::Test => SystemCandidateAction::TestActivation,
        }
    }

    fn effect_class(self) -> SystemEffectClass {
        match self {
            Self::Preview => SystemEffectClass::PreviewHooks,
            Self::Test => SystemEffectClass::TemporaryLiveActivation,
        }
    }

    fn switch_argument(self) -> &'static str {
        match self {
            Self::Preview => "dry-activate",
            Self::Test => "test",
        }
    }

    fn authorization_id(self) -> &'static str {
        match self {
            Self::Preview => "auth:blob-authorized-activation-vm-preview",
            Self::Test => "auth:blob-authorized-activation-vm-test",
        }
    }

    fn operation_id(self) -> &'static str {
        match self {
            Self::Preview => "op:blob-authorized-activation-vm-preview",
            Self::Test => "op:blob-authorized-activation-vm-test",
        }
    }

    fn token(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Test => "test",
        }
    }
}

struct Args {
    sender: String,
    action: RequestedAction,
    candidate: String,
    pkcheck: PathBuf,
}

fn parse_args() -> Result<Args, String> {
    let mut args = env::args().skip(1);
    let mut sender = None;
    let mut action = None;
    let mut candidate = None;
    let mut pkcheck = None;

    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value after {flag}"))?;
        match flag.as_str() {
            "--sender" => sender = Some(value),
            "--action" => action = Some(RequestedAction::parse(&value)?),
            "--candidate" => candidate = Some(value),
            "--pkcheck" => pkcheck = Some(PathBuf::from(value)),
            _ => return Err(format!("unsupported argument: {flag}")),
        }
    }

    Ok(Args {
        sender: sender.ok_or_else(|| "missing --sender".to_owned())?,
        action: action.ok_or_else(|| "missing --action".to_owned())?,
        candidate: candidate.ok_or_else(|| "missing --candidate".to_owned())?,
        pkcheck: pkcheck.ok_or_else(|| "missing --pkcheck".to_owned())?,
    })
}

fn now_unix_ms() -> Result<u64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock before UNIX epoch: {error}"))?;
    u64::try_from(elapsed.as_millis()).map_err(|_| "system clock overflow".to_owned())
}

fn prepared(action: RequestedAction, candidate: &str, now_unix_ms: u64) -> PreparedPrivilegedActivation {
    let node = NodeId::from(LOCAL_NODE);
    let authorization = SystemAuthorizationId::from(action.authorization_id());
    let expires_at = now_unix_ms.saturating_add(60_000);

    PreparedPrivilegedActivation {
        node: node.clone(),
        readiness_observed_at_unix_ms: now_unix_ms,
        authorization: authorization.clone(),
        authorization_expires_at_unix_ms: expires_at,
        prepared_at_unix_ms: now_unix_ms,
        plan: ImmutableNixOsActivationPlan {
            operation_id: SystemOperationId::from(action.operation_id()),
            candidate: SystemCandidateId::from("candidate:blob-authorized-activation-vm"),
            system_spec: SystemSpecId::from("system:blob-authorized-activation-vm"),
            materialization_operation: SystemOperationId::from(
                "op:blob-authorized-activation-vm-materialize",
            ),
            system_closure: candidate.to_owned(),
            action: action.system_action(),
            effect_class: action.effect_class(),
            authority: SystemAuthorityClass::HostAdministrator,
            program: format!("{candidate}/bin/switch-to-configuration"),
            args: vec![action.switch_argument().to_owned()],
            expected_effects: vec![],
            rollback_semantics: "temporary activation; reboot restores boot-default closure".into(),
        },
        readiness_evidence: vec![
            format!("node:{node}"),
            format!("observed-at-unix-ms:{now_unix_ms}"),
        ],
        authorization_evidence: vec![
            format!("authorization:{authorization}"),
            format!("expires-at-unix-ms:{expires_at}"),
        ],
    }
}

/// Test-only diagnostic wrapper around the same production command runner.
/// It does not alter the command, argv, environment, or result. It only exposes
/// already-captured output when switch-to-configuration returns non-zero so the
/// disposable VM CI can diagnose the exact NixOS failure.
struct DiagnosticPrivilegedCommandRunner;

impl PrivilegedCommandRunner for DiagnosticPrivilegedCommandRunner {
    fn run(&self, program: &Path, argument: &str) -> Result<PrivilegedCommandOutcome, String> {
        let outcome = StdPrivilegedCommandRunner.run(program, argument)?;
        if !outcome.succeeded() {
            eprintln!(
                "BLOB_VM_SWITCH_FAILED program={} argument={} exit-code={:?}",
                program.display(),
                argument,
                outcome.exit_code
            );
            if !outcome.stdout.is_empty() {
                eprintln!("BLOB_VM_SWITCH_STDOUT_BEGIN");
                eprintln!("{}", outcome.stdout.trim_end());
                eprintln!("BLOB_VM_SWITCH_STDOUT_END");
            }
            if !outcome.stderr.is_empty() {
                eprintln!("BLOB_VM_SWITCH_STDERR_BEGIN");
                eprintln!("{}", outcome.stderr.trim_end());
                eprintln!("BLOB_VM_SWITCH_STDERR_END");
            }
        }
        Ok(outcome)
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let now = now_unix_ms()?;
    let prepared = prepared(args.action, &args.candidate, now);

    let request = PolkitAuthorizationRequest::for_prepared(args.sender.clone(), &prepared)
        .map_err(|error| format!("authorization request rejected: {error:?}"))?;
    let checker = PkcheckAuthorizationChecker::new(args.pkcheck, StdPkcheckCommandRunner)
        .map_err(|error| format!("pkcheck checker rejected: {error:?}"))?;
    let grant = checker
        .check_user_initiated(&request, now)
        .map_err(|error| format!("polkit authorization rejected: {error:?}"))?;

    let issuer = RootOwnedActivationPermitIssuer::production_default(LOCAL_NODE);
    let issued = issuer
        .issue(&prepared, &grant, now)
        .map_err(|error| format!("trusted permit issuance rejected: {error:?}"))?;

    let permits = FileTrustedActivationPermitStore::production_default();
    let replay = FilePrivilegedExecutionLedger::production_default();
    let host = LocalNixOsActivationHost;
    let runner = DiagnosticPrivilegedCommandRunner;
    let boundary = RootOwnedNixOsActivationBoundary::new(LOCAL_NODE);

    let execution = boundary
        .execute(&prepared, now, &host, &runner, &permits, &replay)
        .map_err(|error| format!("root activation boundary rejected: {error:?}"))?;

    println!("sender={}", grant.system_bus_name());
    println!("action={}", args.action.token());
    println!("authorization={}", issued.authorization);
    println!("permit={}", issued.permit_path.display());
    println!("before={}", execution.before_system_closure);
    println!("after={}", execution.after_system_closure);
    println!("command-succeeded={}", execution.command.succeeded());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("BLOB_AUTHORIZED_ACTIVATION_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
