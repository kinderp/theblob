#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use blob_core::{NodeId, SystemCandidateId, SystemOperationId, SystemSpecId};
use blob_nix_nixos_materialization_authority::{
    MaterializationIntentSpec, NixMaterializationInspector, RootMaterializationAdmissionAuthority,
    StdNixMaterializationInspector,
};

const LOCAL_NODE: &str = "node:blob-prepared-request-daemon-vm";

enum Mode {
    Begin,
    Resume,
    Complete,
}

struct Args {
    mode: Mode,
    operation: SystemOperationId,
    candidate: Option<SystemCandidateId>,
    system_spec: Option<SystemSpecId>,
    source: Option<PathBuf>,
    attribute: Option<String>,
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
        operation: SystemOperationId::from(value(&args, "--operation")?),
        candidate: optional_value(&args, "--candidate").map(SystemCandidateId::from),
        system_spec: optional_value(&args, "--system-spec").map(SystemSpecId::from),
        source: optional_value(&args, "--source").map(PathBuf::from),
        attribute: optional_value(&args, "--attribute"),
        nix: PathBuf::from(value(&args, "--nix")?),
        nix_store: PathBuf::from(value(&args, "--nix-store")?),
    })
}

fn now_unix_ms() -> Result<u64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock before UNIX epoch: {error}"))?;
    u64::try_from(elapsed.as_millis()).map_err(|_| "system clock overflow".to_owned())
}

fn print_intent(intent: &blob_nix_nixos_materialization_authority::MaterializationIntent) {
    println!("operation={}", intent.materialization_operation);
    println!("derivation={}", intent.derivation_path.display());
    println!("expected-output={}", intent.expected_output.display());
    println!("build-target={}", intent.build_target());
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let inspector = StdNixMaterializationInspector::new(args.nix, args.nix_store);
    let authority = RootMaterializationAdmissionAuthority::production_default();

    match args.mode {
        Mode::Begin => {
            let candidate = args.candidate.ok_or_else(|| "missing --candidate".to_owned())?;
            let system_spec = args.system_spec.ok_or_else(|| "missing --system-spec".to_owned())?;
            let source = args.source.ok_or_else(|| "missing --source".to_owned())?;
            let attribute = args.attribute.ok_or_else(|| "missing --attribute".to_owned())?;
            let intent = authority
                .begin(
                    &MaterializationIntentSpec {
                        node: NodeId::from(LOCAL_NODE),
                        candidate,
                        system_spec,
                        materialization_operation: args.operation,
                        immutable_flake_root: source,
                        installable_attribute: attribute,
                        created_at_unix_ms: now_unix_ms()?,
                    },
                    &inspector,
                )
                .map_err(|error| format!("materialization begin rejected: {error:?}"))?;
            print_intent(&intent);
        }
        Mode::Resume => {
            // The caller supplies only the operation id. Root reloads the durable
            // identity, then re-resolves from the *persisted* immutable source and
            // attribute. This may restore Nix's .drv representation after reboot,
            // but recovery succeeds only if both derivation and output remain
            // exactly equal to the identity committed at begin.
            let intent = authority
                .load_pending(&args.operation)
                .map_err(|error| format!("materialization resume rejected: {error:?}"))?;
            let resolved = inspector
                .resolve_exact_derivation(
                    &intent.immutable_flake_root,
                    &intent.installable_attribute,
                )
                .map_err(|error| format!("materialization resume inspection failed: {error}"))?;
            if resolved.derivation_path != intent.derivation_path
                || resolved.expected_output != intent.expected_output
            {
                return Err(format!(
                    "materialization resume identity mismatch: persisted drv={} output={}, resolved drv={} output={}",
                    intent.derivation_path.display(),
                    intent.expected_output.display(),
                    resolved.derivation_path.display(),
                    resolved.expected_output.display(),
                ));
            }
            print_intent(&intent);
        }
        Mode::Complete => {
            let admission = authority
                .complete(&args.operation, now_unix_ms()?, &inspector)
                .map_err(|error| format!("materialization completion rejected: {error:?}"))?;
            println!("operation={}", admission.materialization_operation);
            println!("closure={}", admission.system_closure);
            for evidence in admission.provenance {
                println!("provenance={evidence}");
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("BLOB_MATERIALIZATION_AUTHORITY_REJECTED: {error}");
            ExitCode::FAILURE
        }
    }
}
