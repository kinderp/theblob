use std::path::{Path, PathBuf};

use blob_core::{SystemCandidateAction, SystemCandidateOperation};
use blob_nix_nixos::{NixOsBackend, NixOsCandidateTarget};
use blob_system_executor::{NonPrivilegedNixExecutor, SystemOperationStatus};

fn main() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root must exist");
    let flake_path: PathBuf = repo_root.join("backends/blob-nix-nixos/nix/reference");

    let operation = SystemCandidateOperation::new(
        "op:reference-materialize",
        "candidate:reference",
        "system:linux-pilot",
        SystemCandidateAction::Materialize,
    );
    let target = NixOsCandidateTarget {
        flake_path,
        configuration: "blob-pilot".into(),
    };
    let plan = NixOsBackend::plan_operation(&operation, &target)
        .expect("reference materialization plan must be valid");

    let result = NonPrivilegedNixExecutor::execute(&plan, &repo_root)
        .expect("reference materialization process must start");

    println!("status={:?}", result.status);
    println!("duration_us={}", result.duration_us);
    for store_path in &result.store_paths {
        println!("store_path={store_path}");
    }
    for evidence in result.evidence_lines() {
        println!("evidence={evidence}");
    }

    assert_eq!(result.status, SystemOperationStatus::Succeeded);
    assert!(
        !result.store_paths.is_empty(),
        "successful Nix materialization must report at least one store path"
    );
}
