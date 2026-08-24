#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use blob_core::{BindingLeaseId, ImplementationId, TaskId};

/// MVP-only process-backed Capsule implementation.
///
/// This proves the execution lifecycle but is **not** a security sandbox.
/// Production isolation belongs to later WASM/OCI/microVM backends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalProcessCapsule {
    pub implementation: ImplementationId,
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionRequest {
    pub task: TaskId,
    pub lease: BindingLeaseId,
    pub working_directory: PathBuf,
    pub capsule: LocalProcessCapsule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionStatus {
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionResult {
    pub task: TaskId,
    pub lease: BindingLeaseId,
    pub implementation: ImplementationId,
    pub status: ExecutionStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_us: u64,
}

#[derive(Debug)]
pub enum ExecutionError {
    EmptyProgram,
    Spawn(std::io::Error),
}

pub struct LocalProcessExecutor;

impl LocalProcessExecutor {
    pub fn execute(request: &ExecutionRequest) -> Result<ExecutionResult, ExecutionError> {
        if request.capsule.program.trim().is_empty() {
            return Err(ExecutionError::EmptyProgram);
        }

        let started = Instant::now();
        let output = Command::new(&request.capsule.program)
            .args(&request.capsule.args)
            .current_dir(&request.working_directory)
            .output()
            .map_err(ExecutionError::Spawn)?;
        let duration_us = started.elapsed().as_micros().min(u64::MAX as u128) as u64;

        Ok(ExecutionResult {
            task: request.task.clone(),
            lease: request.lease.clone(),
            implementation: request.capsule.implementation.clone(),
            status: if output.status.success() {
                ExecutionStatus::Succeeded
            } else {
                ExecutionStatus::Failed
            },
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            duration_us,
        })
    }
}
