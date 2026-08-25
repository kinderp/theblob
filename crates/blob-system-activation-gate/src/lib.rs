#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::sync::Mutex;

use blob_core::{
    NodeId, PhysicalTestNodeProfile, PhysicalTestNodeReadiness, PhysicalTestNodeViolation,
    SystemAuthorizationId, SystemAuthorizationViolation, SystemCandidateOperation,
    SystemOperationAuthorization,
};
use blob_nix_nixos_activation::{
    ImmutableActivationError, ImmutableNixOsActivationPlan, ImmutableNixOsActivationPlanner,
    MaterializedNixOsCandidate,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedPrivilegedActivation {
    pub node: NodeId,
    pub readiness_observed_at_unix_ms: u64,
    pub authorization: SystemAuthorizationId,
    pub prepared_at_unix_ms: u64,
    pub plan: ImmutableNixOsActivationPlan,
    pub readiness_evidence: Vec<String>,
    pub authorization_evidence: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivationGateError {
    ReadinessRejected(Vec<PhysicalTestNodeViolation>),
    AuthorizationRejected(Vec<SystemAuthorizationViolation>),
    ActivationPlanRejected(ImmutableActivationError),
    AuthorizationAlreadyConsumed(SystemAuthorizationId),
    AuthorizationLedgerPoisoned,
}

pub trait AuthorizationConsumptionLedger {
    fn consume_once(&self, id: &SystemAuthorizationId) -> Result<(), ActivationGateError>;
}

#[derive(Default)]
pub struct InMemoryAuthorizationLedger {
    consumed: Mutex<BTreeSet<SystemAuthorizationId>>,
}

impl InMemoryAuthorizationLedger {
    pub fn was_consumed(&self, id: &SystemAuthorizationId) -> bool {
        self.consumed
            .lock()
            .map(|consumed| consumed.contains(id))
            .unwrap_or(false)
    }
}

impl AuthorizationConsumptionLedger for InMemoryAuthorizationLedger {
    fn consume_once(&self, id: &SystemAuthorizationId) -> Result<(), ActivationGateError> {
        let mut consumed = self
            .consumed
            .lock()
            .map_err(|_| ActivationGateError::AuthorizationLedgerPoisoned)?;
        if !consumed.insert(id.clone()) {
            return Err(ActivationGateError::AuthorizationAlreadyConsumed(
                id.clone(),
            ));
        }
        Ok(())
    }
}

pub struct PrivilegedActivationGate;

impl PrivilegedActivationGate {
    pub fn prepare<L: AuthorizationConsumptionLedger>(
        operation: &SystemCandidateOperation,
        materialized: &MaterializedNixOsCandidate,
        profile: &PhysicalTestNodeProfile,
        readiness: &PhysicalTestNodeReadiness,
        authorization: &SystemOperationAuthorization,
        now_unix_ms: u64,
        ledger: &L,
    ) -> Result<PreparedPrivilegedActivation, ActivationGateError> {
        profile
            .validate_readiness(&operation.action, readiness)
            .map_err(ActivationGateError::ReadinessRejected)?;

        authorization
            .validate_for(operation, readiness, now_unix_ms)
            .map_err(ActivationGateError::AuthorizationRejected)?;

        let plan = ImmutableNixOsActivationPlanner::plan(operation, materialized)
            .map_err(ActivationGateError::ActivationPlanRejected)?;

        // Consumption happens only after all semantic/readiness/plan checks pass.
        // It intentionally happens before any future privileged execution begins:
        // a crash may waste a receipt, but it cannot safely replay it.
        ledger.consume_once(&authorization.id)?;

        Ok(PreparedPrivilegedActivation {
            node: readiness.node.clone(),
            readiness_observed_at_unix_ms: readiness.observed_at_unix_ms,
            authorization: authorization.id.clone(),
            prepared_at_unix_ms: now_unix_ms,
            plan,
            readiness_evidence: readiness.evidence_lines(),
            authorization_evidence: authorization.evidence_lines(),
        })
    }
}

#[cfg(test)]
mod tests {
    use blob_core::{
        PhysicalNodeSubstrate, SystemArchitecture, SystemAuthorityClass, SystemAuthorizationUsePolicy,
        SystemCandidateAction, SystemCandidateId, SystemEffectClass, SystemOperationId,
        SystemSpecId,
    };
    use blob_system_executor::{SystemOperationResult, SystemOperationStatus};

    use super::*;

    fn operation(action: SystemCandidateAction) -> SystemCandidateOperation {
        SystemCandidateOperation::new(
            "op:activate",
            "candidate:one",
            "system:one",
            action,
        )
    }

    fn materialized() -> MaterializedNixOsCandidate {
        MaterializedNixOsCandidate::from_operation_result(&SystemOperationResult {
            operation_id: SystemOperationId::from("op:materialize"),
            candidate: SystemCandidateId::from("candidate:one"),
            system_spec: SystemSpecId::from("system:one"),
            action: SystemCandidateAction::Materialize,
            effect_class: SystemEffectClass::MaterializationOnly,
            status: SystemOperationStatus::Succeeded,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            duration_us: 1,
            store_paths: vec!["/nix/store/abc123-nixos-system-blob-pilot".into()],
        })
        .expect("materialized candidate")
    }

    fn profile() -> PhysicalTestNodeProfile {
        PhysicalTestNodeProfile::nixos_pilot("node:lab", SystemArchitecture::X86_64)
    }

    fn readiness(observed_at_unix_ms: u64) -> PhysicalTestNodeReadiness {
        PhysicalTestNodeReadiness {
            node: NodeId::from("node:lab"),
            observed_architecture: SystemArchitecture::X86_64,
            observed_substrate: PhysicalNodeSubstrate::NixOs,
            enrolled: true,
            trusted: true,
            on_external_power: true,
            free_space_bytes: 16 * 1024 * 1024 * 1024,
            storage_health_ok: true,
            current_boot_generation: Some("nixos-generation:42".into()),
            rollback_reference: Some("nixos-generation:42".into()),
            local_console_recovery_confirmed: true,
            observed_at_unix_ms,
        }
    }

    fn authorization(
        action: SystemCandidateAction,
        readiness_observed_at_unix_ms: u64,
    ) -> SystemOperationAuthorization {
        SystemOperationAuthorization {
            id: SystemAuthorizationId::from("auth:one"),
            operation: SystemOperationId::from("op:activate"),
            candidate: SystemCandidateId::from("candidate:one"),
            system_spec: SystemSpecId::from("system:one"),
            node: NodeId::from("node:lab"),
            action,
            authority: SystemAuthorityClass::HostAdministrator,
            readiness_observed_at_unix_ms,
            use_policy: SystemAuthorizationUsePolicy::SingleUse,
            granted_by: "user:owner".into(),
            reason: "Approve reviewed physical-node experiment.".into(),
            granted_at_unix_ms: readiness_observed_at_unix_ms + 100,
            expires_at_unix_ms: readiness_observed_at_unix_ms + 60_100,
        }
    }

    #[test]
    fn green_readiness_and_exact_authorization_prepare_test_activation() {
        let observed = 1_000;
        let operation = operation(SystemCandidateAction::TestActivation);
        let ledger = InMemoryAuthorizationLedger::default();
        let receipt = authorization(SystemCandidateAction::TestActivation, observed);

        let prepared = PrivilegedActivationGate::prepare(
            &operation,
            &materialized(),
            &profile(),
            &readiness(observed),
            &receipt,
            2_000,
            &ledger,
        )
        .expect("valid privileged activation gate");

        assert_eq!(prepared.plan.args, vec!["test"]);
        assert!(prepared
            .plan
            .program
            .ends_with("/bin/switch-to-configuration"));
        assert!(ledger.was_consumed(&receipt.id));
    }

    #[test]
    fn authorization_replay_is_rejected() {
        let observed = 1_000;
        let operation = operation(SystemCandidateAction::PreviewActivation);
        let ledger = InMemoryAuthorizationLedger::default();
        let receipt = authorization(SystemCandidateAction::PreviewActivation, observed);

        PrivilegedActivationGate::prepare(
            &operation,
            &materialized(),
            &profile(),
            &readiness(observed),
            &receipt,
            2_000,
            &ledger,
        )
        .expect("first use must succeed");

        assert_eq!(
            PrivilegedActivationGate::prepare(
                &operation,
                &materialized(),
                &profile(),
                &readiness(observed),
                &receipt,
                2_001,
                &ledger,
            ),
            Err(ActivationGateError::AuthorizationAlreadyConsumed(
                SystemAuthorizationId::from("auth:one")
            ))
        );
    }

    #[test]
    fn unsafe_readiness_rejects_without_consuming_authorization() {
        let observed = 1_000;
        let operation = operation(SystemCandidateAction::TestActivation);
        let ledger = InMemoryAuthorizationLedger::default();
        let receipt = authorization(SystemCandidateAction::TestActivation, observed);
        let mut unsafe_readiness = readiness(observed);
        unsafe_readiness.on_external_power = false;

        assert!(matches!(
            PrivilegedActivationGate::prepare(
                &operation,
                &materialized(),
                &profile(),
                &unsafe_readiness,
                &receipt,
                2_000,
                &ledger,
            ),
            Err(ActivationGateError::ReadinessRejected(_))
        ));
        assert!(!ledger.was_consumed(&receipt.id));
    }

    #[test]
    fn stale_readiness_authorization_rejects_without_consumption() {
        let operation = operation(SystemCandidateAction::TestActivation);
        let ledger = InMemoryAuthorizationLedger::default();
        let receipt = authorization(SystemCandidateAction::TestActivation, 1_000);

        assert!(matches!(
            PrivilegedActivationGate::prepare(
                &operation,
                &materialized(),
                &profile(),
                &readiness(1_001),
                &receipt,
                2_000,
                &ledger,
            ),
            Err(ActivationGateError::AuthorizationRejected(_))
        ));
        assert!(!ledger.was_consumed(&receipt.id));
    }

    #[test]
    fn expired_authorization_rejects_without_consumption() {
        let observed = 1_000;
        let operation = operation(SystemCandidateAction::TestActivation);
        let ledger = InMemoryAuthorizationLedger::default();
        let receipt = authorization(SystemCandidateAction::TestActivation, observed);

        assert!(matches!(
            PrivilegedActivationGate::prepare(
                &operation,
                &materialized(),
                &profile(),
                &readiness(observed),
                &receipt,
                61_100,
                &ledger,
            ),
            Err(ActivationGateError::AuthorizationRejected(_))
        ));
        assert!(!ledger.was_consumed(&receipt.id));
    }

    #[test]
    fn gate_never_prepares_persistent_switch_or_boot() {
        let observed = 1_000;
        let operation = operation(SystemCandidateAction::PreviewActivation);
        let ledger = InMemoryAuthorizationLedger::default();
        let receipt = authorization(SystemCandidateAction::PreviewActivation, observed);

        let prepared = PrivilegedActivationGate::prepare(
            &operation,
            &materialized(),
            &profile(),
            &readiness(observed),
            &receipt,
            2_000,
            &ledger,
        )
        .unwrap();

        assert!(prepared.plan.args.iter().all(|arg| arg != "switch" && arg != "boot"));
        assert!(!prepared.plan.program.contains("nixos-rebuild"));
    }
}
