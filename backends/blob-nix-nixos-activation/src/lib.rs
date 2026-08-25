#![forbid(unsafe_code)]

use std::path::{Component, Path};

use blob_core::{
    SystemAuthorityClass, SystemCandidateAction, SystemCandidateId, SystemCandidateOperation,
    SystemEffectClass, SystemOperationId, SystemOperationViolation, SystemSpecId,
};
use blob_system_executor::{SystemOperationResult, SystemOperationStatus};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterializedNixOsCandidate {
    pub candidate: SystemCandidateId,
    pub system_spec: SystemSpecId,
    pub materialization_operation: SystemOperationId,
    pub system_closure: String,
}

impl MaterializedNixOsCandidate {
    pub fn from_operation_result(
        result: &SystemOperationResult,
    ) -> Result<Self, ImmutableActivationError> {
        if result.action != SystemCandidateAction::Materialize
            || result.effect_class != SystemEffectClass::MaterializationOnly
        {
            return Err(ImmutableActivationError::NotSystemMaterialization);
        }
        if result.status != SystemOperationStatus::Succeeded {
            return Err(ImmutableActivationError::MaterializationFailed);
        }
        if result.store_paths.len() != 1 {
            return Err(ImmutableActivationError::ExpectedSingleSystemClosure {
                observed: result.store_paths.len(),
            });
        }

        let system_closure = result.store_paths[0].clone();
        validate_nix_store_closure(&system_closure)?;

        Ok(Self {
            candidate: result.candidate.clone(),
            system_spec: result.system_spec.clone(),
            materialization_operation: result.operation_id.clone(),
            system_closure,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImmutableNixOsActivationPlan {
    pub operation_id: SystemOperationId,
    pub candidate: SystemCandidateId,
    pub system_spec: SystemSpecId,
    pub materialization_operation: SystemOperationId,
    pub system_closure: String,
    pub action: SystemCandidateAction,
    pub effect_class: SystemEffectClass,
    pub authority: SystemAuthorityClass,
    pub program: String,
    pub args: Vec<String>,
    pub expected_effects: Vec<String>,
    pub rollback_semantics: String,
}

impl ImmutableNixOsActivationPlan {
    pub fn changes_live_system(&self) -> bool {
        self.effect_class == SystemEffectClass::TemporaryLiveActivation
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImmutableActivationError {
    InvalidOperation(Vec<SystemOperationViolation>),
    NotSystemMaterialization,
    MaterializationFailed,
    ExpectedSingleSystemClosure { observed: usize },
    InvalidNixStoreClosure,
    CandidateMismatch,
    SystemSpecMismatch,
    UnsupportedAction(SystemCandidateAction),
}

pub struct ImmutableNixOsActivationPlanner;

impl ImmutableNixOsActivationPlanner {
    pub fn plan(
        operation: &SystemCandidateOperation,
        materialized: &MaterializedNixOsCandidate,
    ) -> Result<ImmutableNixOsActivationPlan, ImmutableActivationError> {
        operation
            .validate_policy()
            .map_err(ImmutableActivationError::InvalidOperation)?;
        validate_nix_store_closure(&materialized.system_closure)?;

        if operation.candidate != materialized.candidate {
            return Err(ImmutableActivationError::CandidateMismatch);
        }
        if operation.system_spec != materialized.system_spec {
            return Err(ImmutableActivationError::SystemSpecMismatch);
        }

        let (arg, expected_effects, rollback_semantics) = match operation.action {
            SystemCandidateAction::PreviewActivation => (
                "dry-activate",
                vec![
                    "inspect the running system and print the changes required to reach the already-materialized candidate"
                        .into(),
                    "execute only activation snippets that the candidate explicitly marks as supporting dry activation"
                        .into(),
                    "do not change the boot-default generation".into(),
                ],
                "No configuration switch is performed. Any dry-activation snippet effects must be captured as evidence."
                    .into(),
            ),
            SystemCandidateAction::TestActivation => (
                "test",
                vec![
                    "temporarily switch the running host to the already-materialized immutable candidate closure"
                        .into(),
                    "do not change the boot-default generation".into(),
                ],
                "A reboot returns to the previously recorded boot-default generation; the physical-node readiness record must carry that rollback reference."
                    .into(),
            ),
            other => return Err(ImmutableActivationError::UnsupportedAction(other)),
        };

        let program = format!(
            "{}/bin/switch-to-configuration",
            materialized.system_closure
        );

        Ok(ImmutableNixOsActivationPlan {
            operation_id: operation.id.clone(),
            candidate: operation.candidate.clone(),
            system_spec: operation.system_spec.clone(),
            materialization_operation: materialized.materialization_operation.clone(),
            system_closure: materialized.system_closure.clone(),
            action: operation.action.clone(),
            effect_class: operation.effect_class.clone(),
            authority: operation.authority.clone(),
            program,
            args: vec![arg.into()],
            expected_effects,
            rollback_semantics,
        })
    }
}

fn validate_nix_store_closure(value: &str) -> Result<(), ImmutableActivationError> {
    let path = Path::new(value);
    let components = path.components().collect::<Vec<_>>();

    let valid = matches!(
        components.as_slice(),
        [
            Component::RootDir,
            Component::Normal(nix),
            Component::Normal(store),
            Component::Normal(_closure)
        ] if *nix == "nix" && *store == "store"
    );

    if valid {
        Ok(())
    } else {
        Err(ImmutableActivationError::InvalidNixStoreClosure)
    }
}

#[cfg(test)]
mod tests {
    use blob_core::{SystemCandidateOperation, SystemOperationId, SystemSpecId};

    use super::*;

    fn result() -> SystemOperationResult {
        SystemOperationResult {
            operation_id: SystemOperationId::from("op:materialize"),
            candidate: SystemCandidateId::from("candidate:one"),
            system_spec: SystemSpecId::from("system:one"),
            action: SystemCandidateAction::Materialize,
            effect_class: SystemEffectClass::MaterializationOnly,
            status: SystemOperationStatus::Succeeded,
            exit_code: Some(0),
            stdout: "/nix/store/abc123-nixos-system-blob-pilot\n".into(),
            stderr: String::new(),
            duration_us: 1,
            store_paths: vec!["/nix/store/abc123-nixos-system-blob-pilot".into()],
        }
    }

    fn operation(action: SystemCandidateAction) -> SystemCandidateOperation {
        SystemCandidateOperation::new(
            "op:activate",
            "candidate:one",
            "system:one",
            action,
        )
    }

    #[test]
    fn successful_materialization_becomes_immutable_candidate() {
        let candidate = MaterializedNixOsCandidate::from_operation_result(&result())
            .expect("valid materialization");
        assert_eq!(
            candidate.system_closure,
            "/nix/store/abc123-nixos-system-blob-pilot"
        );
        assert_eq!(
            candidate.materialization_operation,
            SystemOperationId::from("op:materialize")
        );
    }

    #[test]
    fn failed_materialization_cannot_be_activated() {
        let mut failed = result();
        failed.status = SystemOperationStatus::Failed;
        assert_eq!(
            MaterializedNixOsCandidate::from_operation_result(&failed),
            Err(ImmutableActivationError::MaterializationFailed)
        );
    }

    #[test]
    fn multiple_outputs_are_rejected_for_pilot_system_materialization() {
        let mut ambiguous = result();
        ambiguous.store_paths.push("/nix/store/other".into());
        assert_eq!(
            MaterializedNixOsCandidate::from_operation_result(&ambiguous),
            Err(ImmutableActivationError::ExpectedSingleSystemClosure { observed: 2 })
        );
    }

    #[test]
    fn mutable_or_nested_paths_are_not_accepted_as_system_closures() {
        let mut mutable = result();
        mutable.store_paths = vec!["/tmp/candidate".into()];
        assert_eq!(
            MaterializedNixOsCandidate::from_operation_result(&mutable),
            Err(ImmutableActivationError::InvalidNixStoreClosure)
        );

        let mut nested = result();
        nested.store_paths = vec!["/nix/store/abc/bin".into()];
        assert_eq!(
            MaterializedNixOsCandidate::from_operation_result(&nested),
            Err(ImmutableActivationError::InvalidNixStoreClosure)
        );
    }

    #[test]
    fn preview_uses_switch_to_configuration_from_exact_closure() {
        let candidate = MaterializedNixOsCandidate::from_operation_result(&result()).unwrap();
        let plan = ImmutableNixOsActivationPlanner::plan(
            &operation(SystemCandidateAction::PreviewActivation),
            &candidate,
        )
        .expect("preview plan");

        assert_eq!(
            plan.program,
            "/nix/store/abc123-nixos-system-blob-pilot/bin/switch-to-configuration"
        );
        assert_eq!(plan.args, vec!["dry-activate"]);
        assert!(plan.program.starts_with(&candidate.system_closure));
        assert!(!plan.program.contains("nixos-rebuild"));
        assert!(plan.args.iter().all(|arg| arg != "switch" && arg != "boot"));
        assert!(!plan.changes_live_system());
    }

    #[test]
    fn test_activation_uses_same_exact_immutable_closure() {
        let candidate = MaterializedNixOsCandidate::from_operation_result(&result()).unwrap();
        let plan = ImmutableNixOsActivationPlanner::plan(
            &operation(SystemCandidateAction::TestActivation),
            &candidate,
        )
        .expect("test activation plan");

        assert_eq!(plan.args, vec!["test"]);
        assert_eq!(plan.authority, SystemAuthorityClass::HostAdministrator);
        assert!(plan.changes_live_system());
        assert_eq!(plan.system_closure, candidate.system_closure);
    }

    #[test]
    fn activation_for_another_candidate_is_rejected() {
        let candidate = MaterializedNixOsCandidate::from_operation_result(&result()).unwrap();
        let mut op = operation(SystemCandidateAction::TestActivation);
        op.candidate = SystemCandidateId::from("candidate:other");
        assert_eq!(
            ImmutableNixOsActivationPlanner::plan(&op, &candidate),
            Err(ImmutableActivationError::CandidateMismatch)
        );
    }

    #[test]
    fn non_activation_actions_are_rejected() {
        let candidate = MaterializedNixOsCandidate::from_operation_result(&result()).unwrap();
        assert_eq!(
            ImmutableNixOsActivationPlanner::plan(
                &operation(SystemCandidateAction::Materialize),
                &candidate,
            ),
            Err(ImmutableActivationError::UnsupportedAction(
                SystemCandidateAction::Materialize
            ))
        );
    }
}
