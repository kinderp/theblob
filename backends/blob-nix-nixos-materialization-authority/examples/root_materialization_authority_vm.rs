#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use blob_core::{NodeId, SystemCandidateId, SystemOperationId, SystemSpecId};
use blob_nix_nixos_materialization_authority::{
    MaterializationIntentSpec, NixMaterializationInspector, RootMaterializationAdmissionAuthority,
    StdNixMaterializationInspector,
};

const LOCAL_NODE: &str = "node:blob-prepared-request-daemon-vm";
const PENDING_GCROOT_DIR: &str = "/nix/var/nix/gcroots/theblob-materializations";

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

fn hex_text(value: &str) -> String {
    value.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn pending_gcroot_path(operation: &SystemOperationId) -> PathBuf {
    Path::new(PENDING_GCROOT_DIR).join(format!(
        "operation-{}-derivation",
        hex_text(operation.as_str())
    ))
}

fn validate_gcroot_dir() -> Result<(), String> {
    let root = Path::new(PENDING_GCROOT_DIR);
    fs::create_dir_all(root).map_err(|error| format!("failed to create GC-root directory: {error}"))?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("failed to protect GC-root directory: {error}"))?;
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("failed to inspect GC-root directory: {error}"))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("invalid pending materialization GC-root directory".into());
    }
    Ok(())
}

fn retain_pending_derivation(
    operation: &SystemOperationId,
    derivation_path: &Path,
) -> Result<(), String> {
    validate_gcroot_dir()?;
    let root = pending_gcroot_path(operation);
    match fs::symlink_metadata(&root) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                return Err("pending materialization GC root is not a symlink".into());
            }
            let target = fs::read_link(&root)
                .map_err(|error| format!("failed to read pending GC root: {error}"))?;
            if target != derivation_path {
                return Err(format!(
                    "pending materialization GC root mismatch: expected {}, observed {}",
                    derivation_path.display(),
                    target.display(),
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            symlink(derivation_path, &root)
                .map_err(|error| format!("failed to create pending derivation GC root: {error}"))?;
        }
        Err(error) => return Err(format!("failed to inspect pending GC root: {error}")),
    }
    Ok(())
}

fn require_pending_derivation(
    operation: &SystemOperationId,
    derivation_path: &Path,
) -> Result<(), String> {
    validate_gcroot_dir()?;
    let root = pending_gcroot_path(operation);
    let metadata = fs::symlink_metadata(&root)
        .map_err(|error| format!("pending materialization GC root missing: {error}"))?;
    if !metadata.file_type().is_symlink() {
        return Err("pending materialization GC root is not a symlink".into());
    }
    let target = fs::read_link(&root)
        .map_err(|error| format!("failed to read pending GC root: {error}"))?;
    if target != derivation_path {
        return Err(format!(
            "pending materialization GC root mismatch: expected {}, observed {}",
            derivation_path.display(),
            target.display(),
        ));
    }
    if !derivation_path.exists() {
        return Err(format!(
            "pending derivation retained by GC root is unavailable: {}",
            derivation_path.display()
        ));
    }
    Ok(())
}

fn release_pending_derivation(operation: &SystemOperationId) -> Result<(), String> {
    let root = pending_gcroot_path(operation);
    match fs::remove_file(&root) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err("pending materialization GC root disappeared before completion".into())
        }
        Err(error) => Err(format!("failed to release pending derivation GC root: {error}")),
    }
}

fn refresh_flake_archive(nix_program: &Path, flake_root: &Path) -> Result<(), String> {
    let output = Command::new(nix_program)
        .arg("flake")
        .arg("archive")
        .arg("--refresh")
        .arg("--no-write-lock-file")
        .arg(flake_root)
        .stdin(Stdio::null())
        .env_clear()
        .env("HOME", "/root")
        .env("USER", "root")
        .env("LOGNAME", "root")
        .env("LANG", "C")
        .output()
        .map_err(|error| format!("failed to spawn nix flake archive: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "nix flake archive refresh failed with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let nix_program = args.nix.clone();
    let inspector = StdNixMaterializationInspector::new(args.nix, args.nix_store);
    let authority = RootMaterializationAdmissionAuthority::production_default();

    match args.mode {
        Mode::Begin => {
            let candidate = args.candidate.ok_or_else(|| "missing --candidate".to_owned())?;
            let system_spec = args.system_spec.ok_or_else(|| "missing --system-spec".to_owned())?;
            let source = args.source.ok_or_else(|| "missing --source".to_owned())?;
            let attribute = args.attribute.ok_or_else(|| "missing --attribute".to_owned())?;

            // Establish retention before making a pending intent visible. A crash
            // before the intent write can leak an inert GC root, but cannot leave
            // a durable pending operation whose derivation closure is already dead.
            let resolved = inspector
                .resolve_exact_derivation(&source, &attribute)
                .map_err(|error| format!("materialization preflight resolution rejected: {error}"))?;
            retain_pending_derivation(&args.operation, &resolved.derivation_path)?;

            let intent = authority
                .begin(
                    &MaterializationIntentSpec {
                        node: NodeId::from(LOCAL_NODE),
                        candidate,
                        system_spec,
                        materialization_operation: args.operation.clone(),
                        immutable_flake_root: source,
                        installable_attribute: attribute,
                        created_at_unix_ms: now_unix_ms()?,
                    },
                    &inspector,
                )
                .map_err(|error| format!("materialization begin rejected: {error:?}"))?;
            if intent.derivation_path != resolved.derivation_path
                || intent.expected_output != resolved.expected_output
            {
                return Err("materialization identity changed between retention and begin".into());
            }
            print_intent(&intent);
        }
        Mode::Resume => {
            // The caller supplies only the operation id. Root reloads the durable
            // identity and first requires the exact derivation GC root established
            // before begin. Archive refresh/re-resolution then revalidates the
            // persisted immutable source identity without accepting replacement
            // fields from the caller.
            let intent = authority
                .load_pending(&args.operation)
                .map_err(|error| format!("materialization resume rejected: {error:?}"))?;
            require_pending_derivation(&args.operation, &intent.derivation_path)?;
            refresh_flake_archive(&nix_program, &intent.immutable_flake_root)
                .map_err(|error| format!("materialization resume archive failed: {error}"))?;
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
            let pending = authority
                .load_pending(&args.operation)
                .map_err(|error| format!("materialization completion pending load rejected: {error:?}"))?;
            require_pending_derivation(&args.operation, &pending.derivation_path)?;
            let admission = authority
                .complete(&args.operation, now_unix_ms()?, &inspector)
                .map_err(|error| format!("materialization completion rejected: {error:?}"))?;
            if admission.system_closure != pending.expected_output.display().to_string() {
                return Err("completed admission did not preserve pending expected output".into());
            }
            release_pending_derivation(&args.operation)?;
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
