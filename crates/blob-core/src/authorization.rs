use crate::{
    NodeId, PhysicalNodeSubstrate, PhysicalTestNodeReadiness, SystemArchitecture,
    SystemAuthorityClass, SystemAuthorizationId, SystemCandidateAction, SystemCandidateId,
    SystemCandidateOperation, SystemOperationId, SystemSpecId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemAuthorizationUsePolicy {
    SingleUse,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemOperationAuthorization {
    pub id: SystemAuthorizationId,
    pub operation: SystemOperationId,
    pub candidate: SystemCandidateId,
    pub system_spec: SystemSpecId,
    pub node: NodeId,
    pub action: SystemCandidateAction,
    pub authority: SystemAuthorityClass,
    /// Binds authorization to the exact readiness snapshot reviewed before approval.
    pub readiness_observed_at_unix_ms: u64,
    pub use_policy: SystemAuthorizationUsePolicy,
    pub granted_by: String,
    pub reason: String,
    pub granted_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl SystemOperationAuthorization {
    pub fn validate_for(
        &self,
        operation: &SystemCandidateOperation,
        readiness: &PhysicalTestNodeReadiness,
        now_unix_ms: u64,
    ) -> Result<(), Vec<SystemAuthorizationViolation>> {
        let mut violations = Vec::new();

        if self.operation != operation.id {
            violations.push(SystemAuthorizationViolation::OperationMismatch);
        }
        if self.candidate != operation.candidate {
            violations.push(SystemAuthorizationViolation::CandidateMismatch);
        }
        if self.system_spec != operation.system_spec {
            violations.push(SystemAuthorizationViolation::SystemSpecMismatch);
        }
        if self.node != readiness.node {
            violations.push(SystemAuthorizationViolation::NodeMismatch);
        }
        if self.readiness_observed_at_unix_ms != readiness.observed_at_unix_ms {
            violations.push(SystemAuthorizationViolation::ReadinessSnapshotMismatch);
        }
        if self.action != operation.action {
            violations.push(SystemAuthorizationViolation::ActionMismatch);
        }
        if self.authority != operation.authority {
            violations.push(SystemAuthorizationViolation::AuthorityMismatch {
                required: operation.authority.clone(),
                granted: self.authority.clone(),
            });
        }
        if operation.authority != SystemAuthorityClass::HostAdministrator {
            violations.push(SystemAuthorizationViolation::OperationDoesNotRequireHostAdministrator);
        }
        if self.granted_by.trim().is_empty() {
            violations.push(SystemAuthorizationViolation::MissingGrantor);
        }
        if self.reason.trim().is_empty() {
            violations.push(SystemAuthorizationViolation::MissingReason);
        }
        if self.granted_at_unix_ms < self.readiness_observed_at_unix_ms {
            violations.push(SystemAuthorizationViolation::GrantedBeforeReadiness);
        }
        if self.expires_at_unix_ms <= self.granted_at_unix_ms {
            violations.push(SystemAuthorizationViolation::InvalidExpiryWindow);
        }
        if now_unix_ms < self.granted_at_unix_ms {
            violations.push(SystemAuthorizationViolation::NotYetValid);
        }
        if now_unix_ms >= self.expires_at_unix_ms {
            violations.push(SystemAuthorizationViolation::Expired);
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }

    pub fn evidence_lines(&self) -> Vec<String> {
        vec![
            format!("authorization:{}", self.id),
            format!("operation:{}", self.operation),
            format!("candidate:{}", self.candidate),
            format!("system-spec:{}", self.system_spec),
            format!("node:{}", self.node),
            format!("action:{:?}", self.action),
            format!("authority:{:?}", self.authority),
            format!(
                "readiness-observed-at-unix-ms:{}",
                self.readiness_observed_at_unix_ms
            ),
            format!("use-policy:{:?}", self.use_policy),
            format!("granted-by:{}", self.granted_by),
            format!("reason:{}", self.reason),
            format!("granted-at-unix-ms:{}", self.granted_at_unix_ms),
            format!("expires-at-unix-ms:{}", self.expires_at_unix_ms),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemAuthorizationViolation {
    OperationMismatch,
    CandidateMismatch,
    SystemSpecMismatch,
    NodeMismatch,
    ReadinessSnapshotMismatch,
    ActionMismatch,
    AuthorityMismatch {
        required: SystemAuthorityClass,
        granted: SystemAuthorityClass,
    },
    OperationDoesNotRequireHostAdministrator,
    MissingGrantor,
    MissingReason,
    GrantedBeforeReadiness,
    InvalidExpiryWindow,
    NotYetValid,
    Expired,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(action: SystemCandidateAction) -> SystemCandidateOperation {
        SystemCandidateOperation::new(
            "op:physical-test",
            "candidate:one",
            "system:one",
            action,
        )
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

    fn authorization(action: SystemCandidateAction) -> SystemOperationAuthorization {
        SystemOperationAuthorization {
            id: SystemAuthorizationId::from("auth:one"),
            operation: SystemOperationId::from("op:physical-test"),
            candidate: SystemCandidateId::from("candidate:one"),
            system_spec: SystemSpecId::from("system:one"),
            node: NodeId::from("node:lab"),
            action,
            authority: SystemAuthorityClass::HostAdministrator,
            readiness_observed_at_unix_ms: 900,
            use_policy: SystemAuthorizationUsePolicy::SingleUse,
            granted_by: "user:owner".into(),
            reason: "Approve this bounded physical-node experiment after reviewing preflight."
                .into(),
            granted_at_unix_ms: 1_000,
            expires_at_unix_ms: 61_000,
        }
    }

    #[test]
    fn exact_scoped_test_authorization_is_valid() {
        let operation = operation(SystemCandidateAction::TestActivation);
        assert_eq!(
            authorization(SystemCandidateAction::TestActivation).validate_for(
                &operation,
                &readiness(900),
                30_000,
            ),
            Ok(())
        );
    }

    #[test]
    fn preview_authorization_cannot_be_reused_for_test_activation() {
        let operation = operation(SystemCandidateAction::TestActivation);
        let violations = authorization(SystemCandidateAction::PreviewActivation)
            .validate_for(&operation, &readiness(900), 30_000)
            .expect_err("action mismatch must reject authorization reuse");
        assert!(violations.contains(&SystemAuthorizationViolation::ActionMismatch));
    }

    #[test]
    fn authorization_cannot_be_reused_on_another_node() {
        let operation = operation(SystemCandidateAction::TestActivation);
        let mut other_readiness = readiness(900);
        other_readiness.node = NodeId::from("node:other");
        let violations = authorization(SystemCandidateAction::TestActivation)
            .validate_for(&operation, &other_readiness, 30_000)
            .expect_err("node mismatch must reject authorization reuse");
        assert!(violations.contains(&SystemAuthorizationViolation::NodeMismatch));
    }

    #[test]
    fn authorization_cannot_be_reused_after_new_readiness_probe() {
        let operation = operation(SystemCandidateAction::TestActivation);
        let violations = authorization(SystemCandidateAction::TestActivation)
            .validate_for(&operation, &readiness(901), 30_000)
            .expect_err("new readiness snapshot requires new authorization");
        assert!(violations.contains(&SystemAuthorizationViolation::ReadinessSnapshotMismatch));
    }

    #[test]
    fn authorization_cannot_be_reused_for_another_candidate() {
        let mut operation = operation(SystemCandidateAction::TestActivation);
        operation.candidate = SystemCandidateId::from("candidate:two");
        let violations = authorization(SystemCandidateAction::TestActivation)
            .validate_for(&operation, &readiness(900), 30_000)
            .expect_err("candidate mismatch must reject authorization reuse");
        assert!(violations.contains(&SystemAuthorizationViolation::CandidateMismatch));
    }

    #[test]
    fn expired_authorization_is_rejected() {
        let operation = operation(SystemCandidateAction::TestActivation);
        let violations = authorization(SystemCandidateAction::TestActivation)
            .validate_for(&operation, &readiness(900), 61_000)
            .expect_err("expired authorization must be rejected");
        assert!(violations.contains(&SystemAuthorizationViolation::Expired));
    }

    #[test]
    fn future_authorization_is_rejected_until_its_issue_time() {
        let operation = operation(SystemCandidateAction::TestActivation);
        let violations = authorization(SystemCandidateAction::TestActivation)
            .validate_for(&operation, &readiness(900), 999)
            .expect_err("future authorization must be rejected");
        assert!(violations.contains(&SystemAuthorizationViolation::NotYetValid));
    }

    #[test]
    fn receipt_granted_before_reviewed_readiness_is_rejected() {
        let operation = operation(SystemCandidateAction::TestActivation);
        let mut receipt = authorization(SystemCandidateAction::TestActivation);
        receipt.readiness_observed_at_unix_ms = 1_001;
        let violations = receipt
            .validate_for(&operation, &readiness(1_001), 30_000)
            .expect_err("approval must follow the reviewed readiness snapshot");
        assert!(violations.contains(&SystemAuthorizationViolation::GrantedBeforeReadiness));
    }

    #[test]
    fn user_level_materialization_does_not_accept_admin_receipt_as_authority_shortcut() {
        let operation = operation(SystemCandidateAction::Materialize);
        let violations = authorization(SystemCandidateAction::Materialize)
            .validate_for(&operation, &readiness(900), 30_000)
            .expect_err("admin receipt is only modeled for privileged operations");
        assert!(violations.iter().any(|violation| matches!(
            violation,
            SystemAuthorizationViolation::AuthorityMismatch { .. }
        )));
        assert!(violations.contains(
            &SystemAuthorizationViolation::OperationDoesNotRequireHostAdministrator
        ));
    }

    #[test]
    fn empty_grantor_or_reason_is_rejected() {
        let operation = operation(SystemCandidateAction::PreviewActivation);
        let mut receipt = authorization(SystemCandidateAction::PreviewActivation);
        receipt.granted_by.clear();
        receipt.reason.clear();

        let violations = receipt
            .validate_for(&operation, &readiness(900), 30_000)
            .expect_err("authorization provenance is required");
        assert!(violations.contains(&SystemAuthorizationViolation::MissingGrantor));
        assert!(violations.contains(&SystemAuthorizationViolation::MissingReason));
    }
}
