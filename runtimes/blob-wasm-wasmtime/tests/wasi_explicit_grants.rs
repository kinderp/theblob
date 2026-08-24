use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use blob_core::{BindingLeaseId, ImplementationId, TaskId};
use blob_wasm_wasmtime::{
    ExplicitGrantWasiRuntime, FilesystemGrant, FilesystemGrantMode, WasiCommandCapsule,
    WasiCommandExecutionRequest, WasiGrantSet,
};

struct ScratchDir {
    root: PathBuf,
}

impl ScratchDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "theblob-wasi-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create scratch directory");
        Self { root }
    }

    fn workspace(&self) -> PathBuf {
        self.root.join("workspace")
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn fixture_component() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/wasi-fixtures/wasm32-wasip2/release/blob-wasi-fs-probe.wasm");
    fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "missing WASIp2 fixture at {}: {error}; build fixtures/fs-probe with the CI target-dir first",
            path.display()
        )
    })
}

fn request(grants: WasiGrantSet) -> WasiCommandExecutionRequest {
    WasiCommandExecutionRequest {
        task: TaskId::from("task:wasi-fs-probe"),
        lease: BindingLeaseId::from("lease:wasi-fs-probe"),
        capsule: WasiCommandCapsule {
            implementation: ImplementationId::from("impl:wasi-fs-probe"),
            component_bytes: fixture_component(),
        },
        grants,
    }
}

fn grant(path: PathBuf, mode: FilesystemGrantMode) -> WasiGrantSet {
    WasiGrantSet {
        preopened_dirs: vec![FilesystemGrant {
            host_path: path,
            guest_path: "/workspace".into(),
            mode,
        }],
    }
}

#[test]
fn filesystem_is_unavailable_without_an_explicit_grant() {
    let runtime = ExplicitGrantWasiRuntime::new();
    let result = runtime
        .execute_command(&request(WasiGrantSet::default()))
        .expect("WASI command should instantiate even with an empty grant set");

    assert!(!result.success, "guest must fail when /workspace is absent");
    assert!(result.applied_filesystem_grants.is_empty());
}

#[test]
fn read_only_preopen_allows_reading() {
    let scratch = ScratchDir::new("read-only");
    let workspace = scratch.workspace();
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::write(workspace.join("input.txt"), "ro\n").expect("write input");

    let runtime = ExplicitGrantWasiRuntime::new();
    let result = runtime
        .execute_command(&request(grant(
            workspace,
            FilesystemGrantMode::ReadOnly,
        )))
        .expect("read-only grant should materialize");

    assert!(result.success);
    assert_eq!(result.applied_filesystem_grants.len(), 1);
    assert_eq!(
        result.applied_filesystem_grants[0].mode,
        FilesystemGrantMode::ReadOnly
    );
}

#[test]
fn read_only_preopen_blocks_writes() {
    let scratch = ScratchDir::new("read-only-write-attempt");
    let workspace = scratch.workspace();
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::write(workspace.join("input.txt"), "rw\n").expect("write input");

    let runtime = ExplicitGrantWasiRuntime::new();
    let result = runtime
        .execute_command(&request(grant(
            workspace.clone(),
            FilesystemGrantMode::ReadOnly,
        )))
        .expect("read-only grant should materialize");

    assert!(!result.success, "guest write must fail under read-only grant");
    assert!(!workspace.join("output.txt").exists());
}

#[test]
fn read_write_preopen_allows_scoped_write() {
    let scratch = ScratchDir::new("read-write");
    let workspace = scratch.workspace();
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::write(workspace.join("input.txt"), "rw\n").expect("write input");

    let runtime = ExplicitGrantWasiRuntime::new();
    let result = runtime
        .execute_command(&request(grant(
            workspace.clone(),
            FilesystemGrantMode::ReadWrite,
        )))
        .expect("read-write grant should materialize");

    assert!(result.success);
    assert_eq!(
        fs::read_to_string(workspace.join("output.txt")).expect("guest output"),
        "blob-write-ok\n"
    );
}

#[test]
fn preopen_cannot_escape_to_parent_directory() {
    let scratch = ScratchDir::new("escape");
    let workspace = scratch.workspace();
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::write(workspace.join("input.txt"), "escape\n").expect("write input");
    fs::write(scratch.root.join("outside.txt"), "must-not-be-visible\n")
        .expect("write outside sentinel");

    let runtime = ExplicitGrantWasiRuntime::new();
    let result = runtime
        .execute_command(&request(grant(
            workspace,
            FilesystemGrantMode::ReadOnly,
        )))
        .expect("read-only grant should materialize");

    assert!(
        result.success,
        "probe exits successfully only when ../outside.txt is not readable"
    );
}
