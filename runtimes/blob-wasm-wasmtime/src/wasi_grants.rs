use std::path::PathBuf;
use std::time::Instant;

use blob_core::{BindingLeaseId, ImplementationId, TaskId};
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Engine, Store};
use wasmtime_wasi::p2::bindings::sync::Command;
use wasmtime_wasi::{FsPerms, WasiCtx, WasiCtxView, WasiView};

pub const WASI_RUNTIME_ID: &str = "wasmtime-wasip2@48";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilesystemGrantMode {
    ReadOnly,
    ReadWrite,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilesystemGrant {
    pub host_path: PathBuf,
    pub guest_path: String,
    pub mode: FilesystemGrantMode,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WasiGrantSet {
    pub preopened_dirs: Vec<FilesystemGrant>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasiCommandCapsule {
    pub implementation: ImplementationId,
    pub component_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasiCommandExecutionRequest {
    pub task: TaskId,
    pub lease: BindingLeaseId,
    pub capsule: WasiCommandCapsule,
    pub grants: WasiGrantSet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedFilesystemGrant {
    pub guest_path: String,
    pub mode: FilesystemGrantMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasiCommandExecutionResult {
    pub task: TaskId,
    pub lease: BindingLeaseId,
    pub implementation: ImplementationId,
    pub runtime: String,
    pub success: bool,
    pub duration_us: u64,
    pub applied_filesystem_grants: Vec<AppliedFilesystemGrant>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WasiRuntimeStage {
    Compile,
    Link,
    GrantMaterialization,
    Instantiate,
    Execute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasiRuntimeError {
    pub stage: WasiRuntimeStage,
    pub message: String,
}

struct WasiState {
    ctx: WasiCtx,
    table: ResourceTable,
}

impl WasiView for WasiState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

pub struct ExplicitGrantWasiRuntime {
    engine: Engine,
}

impl Default for ExplicitGrantWasiRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ExplicitGrantWasiRuntime {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    pub fn execute_command(
        &self,
        request: &WasiCommandExecutionRequest,
    ) -> Result<WasiCommandExecutionResult, WasiRuntimeError> {
        let component = Component::new(&self.engine, &request.capsule.component_bytes).map_err(
            |error| WasiRuntimeError {
                stage: WasiRuntimeStage::Compile,
                message: error.to_string(),
            },
        )?;

        let mut linker = Linker::<WasiState>::new(&self.engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|error| WasiRuntimeError {
            stage: WasiRuntimeStage::Link,
            message: error.to_string(),
        })?;

        let mut builder = WasiCtx::builder();
        builder
            .allow_tcp(false)
            .allow_udp(false)
            .allow_ip_name_lookup(false);

        let mut applied_filesystem_grants = Vec::with_capacity(request.grants.preopened_dirs.len());
        for grant in &request.grants.preopened_dirs {
            let perms = match grant.mode {
                FilesystemGrantMode::ReadOnly => FsPerms::ReadOnly,
                FilesystemGrantMode::ReadWrite => FsPerms::ReadWrite,
            };

            builder
                .preopened_dir(&grant.host_path, &grant.guest_path, perms)
                .map_err(|error| WasiRuntimeError {
                    stage: WasiRuntimeStage::GrantMaterialization,
                    message: format!(
                        "failed to materialize filesystem grant {} -> {}: {error}",
                        grant.host_path.display(),
                        grant.guest_path
                    ),
                })?;

            applied_filesystem_grants.push(AppliedFilesystemGrant {
                guest_path: grant.guest_path.clone(),
                mode: grant.mode.clone(),
            });
        }

        let mut store = Store::new(
            &self.engine,
            WasiState {
                ctx: builder.build(),
                table: ResourceTable::new(),
            },
        );

        let command = Command::instantiate(&mut store, &component, &linker).map_err(|error| {
            WasiRuntimeError {
                stage: WasiRuntimeStage::Instantiate,
                message: error.to_string(),
            }
        })?;

        let started = Instant::now();
        let program_result = command
            .wasi_cli_run()
            .call_run(&mut store)
            .map_err(|error| WasiRuntimeError {
                stage: WasiRuntimeStage::Execute,
                message: error.to_string(),
            })?;
        let duration_us = started.elapsed().as_micros().min(u64::MAX as u128) as u64;

        Ok(WasiCommandExecutionResult {
            task: request.task.clone(),
            lease: request.lease.clone(),
            implementation: request.capsule.implementation.clone(),
            runtime: WASI_RUNTIME_ID.into(),
            success: program_result.is_ok(),
            duration_us,
            applied_filesystem_grants,
        })
    }
}
