#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use blob_core::{
    SystemAuthorityClass, SystemCandidateAction, SystemCandidateId, SystemEffectClass,
    SystemOperationId, SystemSpecId,
};
use blob_nix_nixos::NixCommandPlan;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemOperationStatus {
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemOperationResult {
    pub operation_id: SystemOperationId,
    pub candidate: SystemCandidateId,
    pub system_spec: SystemSpecId,
    pub action: SystemCandidateAction,
    pub effect_class: SystemEffectClass,
    pub status: SystemOperationStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_us: u64,
    pub store_paths: Vec<String>,
}

impl SystemOperationResult {
    pub fn evidence_lines(&self) -> Vec<String> {
        let mut evidence = vec![
            format!("system-operation:{}", self.operation_id),
            format!("candidate:{}", self.candidate),
            format!("system-spec:{}", self.system_spec),
            format!("action:{:?}", self.action),
            format!("effect-class:{:?}", self.effect_class),
            format!("status:{:?}", self.status),
            format!("duration-us:{}", self.duration_us),
        ];

        if let Some(exit_code) = self.exit_code {
            evidence.push(format!("exit-code:{exit_code}"));
        }
        for path in &self.store_paths {
            evidence.push(format!("nix-store-path:{path}"));
        }
        evidence
    }
}

#[derive(Debug)]
pub enum SystemExecutionError {
    PrivilegedOperationRejected,
    LiveSystemOperationRejected,
    UnsupportedAction,
    MalformedPlan(&'static str),
    Spawn(std::io::Error),
}

pub struct NonPrivilegedNixExecutor;

impl NonPrivilegedNixExecutor {
    pub fn execute(
        plan: &NixCommandPlan,
        working_directory: &Path,
    ) -> Result<SystemOperationResult, SystemExecutionError> {
        validate_non_privileged_plan(plan)?;

        let started = Instant::now();
        let output = Command::new(&plan.program)
            .args(&plan.args)
            .current_dir(working_directory)
            .output()
            .map_err(SystemExecutionError::Spawn)?;
        let duration_us = started.elapsed().as_micros().min(u64::MAX as u128) as u64;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let store_paths = parse_store_paths(&stdout);

        Ok(SystemOperationResult {
            operation_id: plan.operation_id.clone(),
            candidate: plan.candidate.clone(),
            system_spec: plan.system_spec.clone(),
            action: plan.action.clone(),
            effect_class: plan.effect_class.clone(),
            status: if output.status.success() {
                SystemOperationStatus::Succeeded
            } else {
                SystemOperationStatus::Failed
            },
            exit_code: output.status.code(),
            stdout,
            stderr,
            duration_us,
            store_paths,
        })
    }
}

fn validate_non_privileged_plan(plan: &NixCommandPlan) -> Result<(), SystemExecutionError> {
    if plan.authority != SystemAuthorityClass::User {
        return Err(SystemExecutionError::PrivilegedOperationRejected);
    }
    if plan.effect_class != SystemEffectClass::MaterializationOnly {
        return Err(SystemExecutionError::LiveSystemOperationRejected);
    }

    let expected_suffix = match plan.action {
        SystemCandidateAction::Materialize => ".config.system.build.toplevel",
        SystemCandidateAction::BuildIsolatedVm => ".config.system.build.vm",
        SystemCandidateAction::PreviewActivation | SystemCandidateAction::TestActivation => {
            return Err(SystemExecutionError::UnsupportedAction);
        }
    };

    if plan.program != "nix" {
        return Err(SystemExecutionError::MalformedPlan(
            "non-privileged executor only permits the nix executable",
        ));
    }

    if plan.args.len() != 4
        || plan.args[0] != "build"
        || plan.args[1] != "--no-link"
        || plan.args[2] != "--print-out-paths"
    {
        return Err(SystemExecutionError::MalformedPlan(
            "unexpected nix build argument shape",
        ));
    }

    if plan.args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "--impure" | "--expr" | "--command" | "switch" | "boot" | "test" | "dry-activate"
        )
    }) {
        return Err(SystemExecutionError::MalformedPlan(
            "forbidden argument in non-privileged system plan",
        ));
    }

    if !plan.args[3].ends_with(expected_suffix) {
        return Err(SystemExecutionError::MalformedPlan(
            "derivation selector does not match semantic action",
        ));
    }

    Ok(())
}

fn parse_store_paths(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("/nix/store/"))
        .map(str::to_owned)
        .collect()
}

pub fn default_reference_working_directory() -> PathBuf {
    PathBuf::from(".")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use blob_core::{SystemCandidateOperation, SystemCandidateAction};
    use blob_nix_nixos::{NixOsBackend, NixOsCandidateTarget};

    use super::*;

    fn target() -> NixOsCandidateTarget {
        NixOsCandidateTarget {
            flake_path: PathBuf::from("/tmp/blob-candidate"),
            configuration: "blob-pilot".into(),
        }
    }

    fn plan(action: SystemCandidateAction) -> NixCommandPlan {
        let operation = SystemCandidateOperation::new(
            "op:test",
            "candidate:test",
            "system:test",
            action,
        );
        NixOsBackend::plan_operation(&operation, &target()).expect("plan must be valid")
    }

    #[test]
    fn materialize_plan_is_accepted() {
        assert!(validate_non_privileged_plan(&plan(SystemCandidateAction::Materialize)).is_ok());
    }

    #[test]
    fn vm_plan_is_accepted() {
        assert!(
            validate_non_privileged_plan(&plan(SystemCandidateAction::BuildIsolatedVm)).is_ok()
        );
    }

    #[test]
    fn preview_activation_is_rejected() {
        assert!(matches!(
            validate_non_privileged_plan(&plan(SystemCandidateAction::PreviewActivation)),
            Err(SystemExecutionError::PrivilegedOperationRejected)
        ));
    }

    #[test]
    fn test_activation_is_rejected() {
        assert!(matches!(
            validate_non_privileged_plan(&plan(SystemCandidateAction::TestActivation)),
            Err(SystemExecutionError::PrivilegedOperationRejected)
        ));
    }

    #[test]
    fn forged_program_is_rejected() {
        let mut candidate = plan(SystemCandidateAction::Materialize);
        candidate.program = "sh".into();
        assert!(matches!(
            validate_non_privileged_plan(&candidate),
            Err(SystemExecutionError::MalformedPlan(_))
        ));
    }

    #[test]
    fn impure_build_flag_is_rejected() {
        let mut candidate = plan(SystemCandidateAction::Materialize);
        candidate.args.insert(1, "--impure".into());
        assert!(matches!(
            validate_non_privileged_plan(&candidate),
            Err(SystemExecutionError::MalformedPlan(_))
        ));
    }

    #[test]
    fn mismatched_derivation_is_rejected() {
        let mut candidate = plan(SystemCandidateAction::Materialize);
        candidate.args[3] = "/tmp/x#nixosConfigurations.blob.config.system.build.vm".into();
        assert!(matches!(
            validate_non_privileged_plan(&candidate),
            Err(SystemExecutionError::MalformedPlan(_))
        ));
    }

    #[test]
    fn store_paths_are_extracted_from_structured_output() {
        assert_eq!(
            parse_store_paths("warning\n/nix/store/abc-one\n/nix/store/def-two\n"),
            vec!["/nix/store/abc-one", "/nix/store/def-two"]
        );
    }
}
