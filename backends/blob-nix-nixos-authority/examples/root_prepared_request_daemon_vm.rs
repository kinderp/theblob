#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use blob_core::{NodeId, SystemAuthorizationId};
use blob_nix_nixos_authority::{
    PkcheckAuthorizationChecker, PolkitAuthorizationRequest, RootOwnedActivationPermitIssuer,
    StdPkcheckCommandRunner,
};
use blob_nix_nixos_privileged_helper::{
    FilePrivilegedExecutionLedger, LocalNixOsActivationHost, PrivilegedNixOsActivationHelper,
    StdPrivilegedCommandRunner,
};
use blob_nix_nixos_request_store::{
    FilePreparedActivationRequestStore, PreparedActivationRequestStoreError,
};
use blob_nix_nixos_root_boundary::{
    FileTrustedActivationPermitStore, RootOwnedNixOsActivationBoundary,
};

const LOCAL_NODE: &str = "node:blob-prepared-request-daemon-vm";
const FAULT_EXIT_CODE: u8 = 70;

struct Args {
    sender: String,
    authorization: SystemAuthorizationId,
    pkcheck: PathBuf,
    fault_after_claim: bool,
}

enum RunFailure {
    Message(String),
    FaultAfterClaim,
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

    let sender = value(&args, "--sender")?;
    let authorization = SystemAuthorizationId::from(value(&args, "--authorization")?);
    let pkcheck = PathBuf::from(value(&args, "--pkcheck")?);
    let fault_after_claim = args.iter().any(|arg| arg == "--fault-after-claim");

    Ok(Args {
        sender,
        authorization,
        pkcheck,
        fault_after_claim,
    })
}

fn now_unix_ms() -> Result<u64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock before UNIX epoch: {error}"))?;
    u64::try_from(elapsed.as_millis()).map_err(|_| "system clock overflow".to_owned())
}

fn mark_failed(
    requests: &FilePreparedActivationRequestStore,
    claimed: &blob_nix_nixos_request_store::ClaimedPreparedActivationRequest,
    original: String,
) -> RunFailure {
    match requests.mark_failed(claimed) {
        Ok(()) => RunFailure::Message(original),
        Err(error) => RunFailure::Message(format!(
            "{original}; additionally failed to quarantine claimed request: {error:?}"
        )),
    }
}

fn run(args: Args) -> Result<(), RunFailure> {
    let requests = FilePreparedActivationRequestStore::production_default();
    let prepared = requests
        .load_ready(&args.authorization)
        .map_err(|error| RunFailure::Message(format!("prepared request load rejected: {error:?}")))?;

    let now = now_unix_ms().map_err(RunFailure::Message)?;
    PrivilegedNixOsActivationHelper::new(NodeId::from(LOCAL_NODE))
        .validate_prepared(&prepared, now)
        .map_err(|error| {
            RunFailure::Message(format!("prepared request semantic validation rejected: {error:?}"))
        })?;

    let request = PolkitAuthorizationRequest::for_prepared(args.sender, &prepared)
        .map_err(|error| RunFailure::Message(format!("authorization request rejected: {error:?}")))?;
    let checker = PkcheckAuthorizationChecker::new(args.pkcheck, StdPkcheckCommandRunner)
        .map_err(|error| RunFailure::Message(format!("pkcheck checker rejected: {error:?}")))?;
    let grant = checker
        .check_user_initiated(&request, now)
        .map_err(|error| RunFailure::Message(format!("polkit authorization rejected: {error:?}")))?;

    // Claim only after successful OS authorization. A denied caller cannot spend
    // or strand the prepared request. The claim implementation re-reads the exact
    // request after the durable claim receipt, closing the authorization TOCTOU.
    let claimed = requests
        .claim_exact(&prepared)
        .map_err(|error| RunFailure::Message(format!("prepared request claim rejected: {error:?}")))?;

    if args.fault_after_claim {
        eprintln!(
            "BLOB_PREPARED_REQUEST_FAULT_AFTER_CLAIM authorization={}",
            prepared.authorization
        );
        return Err(RunFailure::FaultAfterClaim);
    }

    let now_after_authorization = now_unix_ms().map_err(|error| {
        mark_failed(&requests, &claimed, format!("clock observation failed: {error}"))
    })?;
    let issuer = RootOwnedActivationPermitIssuer::production_default(LOCAL_NODE);
    let issued = issuer.issue(&prepared, &grant, now_after_authorization).map_err(|error| {
        mark_failed(
            &requests,
            &claimed,
            format!("trusted permit issuance rejected: {error:?}"),
        )
    })?;

    let permits = FileTrustedActivationPermitStore::production_default();
    let replay = FilePrivilegedExecutionLedger::production_default();
    let host = LocalNixOsActivationHost;
    let runner = StdPrivilegedCommandRunner;
    let boundary = RootOwnedNixOsActivationBoundary::new(LOCAL_NODE);

    let execution = boundary
        .execute(
            &prepared,
            now_after_authorization,
            &host,
            &runner,
            &permits,
            &replay,
        )
        .map_err(|error| {
            mark_failed(
                &requests,
                &claimed,
                format!("root activation boundary rejected: {error:?}"),
            )
        })?;

    requests.mark_completed(&claimed).map_err(|error| {
        RunFailure::Message(format!(
            "activation succeeded but request completion receipt failed: {error:?}"
        ))
    })?;

    println!("sender={}", grant.system_bus_name());
    println!("authorization={}", issued.authorization);
    println!("request-state=completed");
    println!("permit={}", issued.permit_path.display());
    println!("before={}", execution.before_system_closure);
    println!("after={}", execution.after_system_closure);
    println!("command-succeeded={}", execution.command.succeeded());
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("BLOB_PREPARED_REQUEST_DAEMON_REJECTED: {error}");
            return ExitCode::FAILURE;
        }
    };

    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(RunFailure::FaultAfterClaim) => ExitCode::from(FAULT_EXIT_CODE),
        Err(RunFailure::Message(error)) => {
            eprintln!("BLOB_PREPARED_REQUEST_DAEMON_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
