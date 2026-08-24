#![deny(unsafe_code)]

use std::time::Instant;

use blob_core::{BindingLeaseId, ImplementationId, TaskId};
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};

pub const RUNTIME_ID: &str = "wasmtime-component@49";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasmComponentCapsule {
    pub implementation: ImplementationId,
    pub component_bytes: Vec<u8>,
    pub entrypoint: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentExecutionRequest {
    pub task: TaskId,
    pub lease: BindingLeaseId,
    pub capsule: WasmComponentCapsule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentExecutionResult {
    pub task: TaskId,
    pub lease: BindingLeaseId,
    pub implementation: ImplementationId,
    pub runtime: String,
    pub return_code: u32,
    pub duration_us: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeStage {
    Compile,
    Instantiate,
    ExportLookup,
    Execute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeError {
    pub stage: RuntimeStage,
    pub message: String,
}

pub struct DenyByDefaultComponentRuntime {
    engine: Engine,
}

impl Default for DenyByDefaultComponentRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl DenyByDefaultComponentRuntime {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    pub fn execute_u32(
        &self,
        request: &ComponentExecutionRequest,
    ) -> Result<ComponentExecutionResult, RuntimeError> {
        let component = Component::new(&self.engine, &request.capsule.component_bytes)
            .map_err(|error| RuntimeError {
                stage: RuntimeStage::Compile,
                message: error.to_string(),
            })?;

        // Intentionally empty. Phase 2A provides no WASI and no Blob host
        // capabilities. Every host import is denied unless explicitly linked
        // by a later grant-aware runtime.
        let linker = Linker::<()>::new(&self.engine);
        let mut store = Store::new(&self.engine, ());

        let instance = linker
            .instantiate(&mut store, &component)
            .map_err(|error| RuntimeError {
                stage: RuntimeStage::Instantiate,
                message: error.to_string(),
            })?;

        let run = instance
            .get_typed_func::<(), (u32,)>(&mut store, request.capsule.entrypoint.as_str())
            .map_err(|error| RuntimeError {
                stage: RuntimeStage::ExportLookup,
                message: error.to_string(),
            })?;

        let started = Instant::now();
        let (return_code,) = run.call(&mut store, ()).map_err(|error| RuntimeError {
            stage: RuntimeStage::Execute,
            message: error.to_string(),
        })?;
        let duration_us = started.elapsed().as_micros().min(u64::MAX as u128) as u64;

        Ok(ComponentExecutionResult {
            task: request.task.clone(),
            lease: request.lease.clone(),
            implementation: request.capsule.implementation.clone(),
            runtime: RUNTIME_ID.into(),
            return_code,
            duration_us,
        })
    }
}
