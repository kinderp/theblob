use crate::{NodeId, SystemArchitecture, SystemCandidateAction};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhysicalNodeSubstrate {
    NixOs,
    LinuxHosted,
    MacOsHosted,
    WindowsHosted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalTestNodeProfile {
    pub node: NodeId,
    pub expected_architecture: SystemArchitecture,
    pub expected_substrate: PhysicalNodeSubstrate,
    pub minimum_free_space_bytes: u64,
    pub allow_preview_activation: bool,
    pub allow_test_activation: bool,
    pub require_external_power_for_live_activation: bool,
    pub require_local_console_for_live_activation: bool,
    pub require_known_boot_rollback_for_live_activation: bool,
}

impl PhysicalTestNodeProfile {
    pub fn nixos_pilot(node: impl Into<NodeId>, architecture: SystemArchitecture) -> Self {
        Self {
            node: node.into(),
            expected_architecture: architecture,
            expected_substrate: PhysicalNodeSubstrate::NixOs,
            minimum_free_space_bytes: 8 * 1024 * 1024 * 1024,
            allow_preview_activation: true,
            allow_test_activation: true,
            require_external_power_for_live_activation: true,
            require_local_console_for_live_activation: true,
            require_known_boot_rollback_for_live_activation: true,
        }
    }

    pub fn validate_readiness(
        &self,
        action: &SystemCandidateAction,
        readiness: &PhysicalTestNodeReadiness,
    ) -> Result<(), Vec<PhysicalTestNodeViolation>> {
        let mut violations = Vec::new();

        if readiness.node != self.node {
            violations.push(PhysicalTestNodeViolation::NodeMismatch);
        }
        if readiness.observed_architecture != self.expected_architecture {
            violations.push(PhysicalTestNodeViolation::ArchitectureMismatch);
        }
        if readiness.observed_substrate != self.expected_substrate {
            violations.push(PhysicalTestNodeViolation::SubstrateMismatch);
        }
        if !readiness.enrolled {
            violations.push(PhysicalTestNodeViolation::NotEnrolled);
        }
        if !readiness.trusted {
            violations.push(PhysicalTestNodeViolation::NotTrusted);
        }
        if !readiness.storage_health_ok {
            violations.push(PhysicalTestNodeViolation::StorageHealthNotConfirmed);
        }
        if readiness.free_space_bytes < self.minimum_free_space_bytes {
            violations.push(PhysicalTestNodeViolation::InsufficientFreeSpace {
                required_bytes: self.minimum_free_space_bytes,
                observed_bytes: readiness.free_space_bytes,
            });
        }

        match action {
            SystemCandidateAction::Materialize | SystemCandidateAction::BuildIsolatedVm => {}
            SystemCandidateAction::PreviewActivation => {
                if !self.allow_preview_activation {
                    violations.push(PhysicalTestNodeViolation::PreviewActivationDisabled);
                }
            }
            SystemCandidateAction::TestActivation => {
                if !self.allow_test_activation {
                    violations.push(PhysicalTestNodeViolation::TestActivationDisabled);
                }
                if self.require_external_power_for_live_activation && !readiness.on_external_power {
                    violations.push(PhysicalTestNodeViolation::ExternalPowerRequired);
                }
                if self.require_local_console_for_live_activation
                    && !readiness.local_console_recovery_confirmed
                {
                    violations.push(PhysicalTestNodeViolation::LocalConsoleRecoveryNotConfirmed);
                }
                if self.require_known_boot_rollback_for_live_activation {
                    if readiness.current_boot_generation.is_none() {
                        violations.push(PhysicalTestNodeViolation::CurrentBootGenerationUnknown);
                    }
                    if readiness.rollback_reference.is_none() {
                        violations.push(PhysicalTestNodeViolation::RollbackReferenceMissing);
                    }
                }
            }
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalTestNodeReadiness {
    pub node: NodeId,
    pub observed_architecture: SystemArchitecture,
    pub observed_substrate: PhysicalNodeSubstrate,
    pub enrolled: bool,
    pub trusted: bool,
    pub on_external_power: bool,
    pub free_space_bytes: u64,
    pub storage_health_ok: bool,
    pub current_boot_generation: Option<String>,
    pub rollback_reference: Option<String>,
    pub local_console_recovery_confirmed: bool,
    pub observed_at_unix_ms: u64,
}

impl PhysicalTestNodeReadiness {
    pub fn evidence_lines(&self) -> Vec<String> {
        vec![
            format!("node:{}", self.node),
            format!("architecture:{:?}", self.observed_architecture),
            format!("substrate:{:?}", self.observed_substrate),
            format!("enrolled:{}", self.enrolled),
            format!("trusted:{}", self.trusted),
            format!("external-power:{}", self.on_external_power),
            format!("free-space-bytes:{}", self.free_space_bytes),
            format!("storage-health-ok:{}", self.storage_health_ok),
            format!(
                "current-boot-generation:{}",
                self.current_boot_generation.as_deref().unwrap_or("unknown")
            ),
            format!(
                "rollback-reference:{}",
                self.rollback_reference.as_deref().unwrap_or("none")
            ),
            format!(
                "local-console-recovery:{}",
                self.local_console_recovery_confirmed
            ),
            format!("observed-at-unix-ms:{}", self.observed_at_unix_ms),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhysicalTestNodeViolation {
    NodeMismatch,
    ArchitectureMismatch,
    SubstrateMismatch,
    NotEnrolled,
    NotTrusted,
    StorageHealthNotConfirmed,
    InsufficientFreeSpace {
        required_bytes: u64,
        observed_bytes: u64,
    },
    PreviewActivationDisabled,
    TestActivationDisabled,
    ExternalPowerRequired,
    LocalConsoleRecoveryNotConfirmed,
    CurrentBootGenerationUnknown,
    RollbackReferenceMissing,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> PhysicalTestNodeProfile {
        PhysicalTestNodeProfile::nixos_pilot("node:lab", SystemArchitecture::X86_64)
    }

    fn ready() -> PhysicalTestNodeReadiness {
        PhysicalTestNodeReadiness {
            node: NodeId::from("node:lab"),
            observed_architecture: SystemArchitecture::X86_64,
            observed_substrate: PhysicalNodeSubstrate::NixOs,
            enrolled: true,
            trusted: true,
            on_external_power: true,
            free_space_bytes: 32 * 1024 * 1024 * 1024,
            storage_health_ok: true,
            current_boot_generation: Some("nixos-generation:42".into()),
            rollback_reference: Some("nixos-generation:41".into()),
            local_console_recovery_confirmed: true,
            observed_at_unix_ms: 1_787_636_000_000,
        }
    }

    #[test]
    fn materialization_needs_trust_and_capacity_but_not_live_activation_recovery() {
        let mut observation = ready();
        observation.on_external_power = false;
        observation.current_boot_generation = None;
        observation.rollback_reference = None;
        observation.local_console_recovery_confirmed = false;

        assert_eq!(
            profile().validate_readiness(&SystemCandidateAction::Materialize, &observation),
            Ok(())
        );
    }

    #[test]
    fn test_activation_requires_power_console_and_known_rollback() {
        let mut observation = ready();
        observation.on_external_power = false;
        observation.current_boot_generation = None;
        observation.rollback_reference = None;
        observation.local_console_recovery_confirmed = false;

        let violations = profile()
            .validate_readiness(&SystemCandidateAction::TestActivation, &observation)
            .expect_err("unsafe live activation must be rejected");

        assert!(violations.contains(&PhysicalTestNodeViolation::ExternalPowerRequired));
        assert!(
            violations.contains(&PhysicalTestNodeViolation::LocalConsoleRecoveryNotConfirmed)
        );
        assert!(violations.contains(&PhysicalTestNodeViolation::CurrentBootGenerationUnknown));
        assert!(violations.contains(&PhysicalTestNodeViolation::RollbackReferenceMissing));
    }

    #[test]
    fn untrusted_node_is_rejected_even_for_non_live_builds() {
        let mut observation = ready();
        observation.trusted = false;

        assert!(matches!(
            profile().validate_readiness(&SystemCandidateAction::BuildIsolatedVm, &observation),
            Err(violations) if violations.contains(&PhysicalTestNodeViolation::NotTrusted)
        ));
    }

    #[test]
    fn platform_mismatch_is_explicit() {
        let mut observation = ready();
        observation.observed_substrate = PhysicalNodeSubstrate::LinuxHosted;

        assert!(matches!(
            profile().validate_readiness(&SystemCandidateAction::Materialize, &observation),
            Err(violations) if violations.contains(&PhysicalTestNodeViolation::SubstrateMismatch)
        ));
    }

    #[test]
    fn insufficient_space_is_reported_with_required_and_observed_values() {
        let mut observation = ready();
        observation.free_space_bytes = 1024;

        assert!(matches!(
            profile().validate_readiness(&SystemCandidateAction::Materialize, &observation),
            Err(violations) if violations.iter().any(|violation| matches!(
                violation,
                PhysicalTestNodeViolation::InsufficientFreeSpace {
                    required_bytes,
                    observed_bytes: 1024
                } if *required_bytes == 8 * 1024 * 1024 * 1024
            ))
        ));
    }

    #[test]
    fn safe_nixos_test_activation_is_accepted() {
        assert_eq!(
            profile().validate_readiness(&SystemCandidateAction::TestActivation, &ready()),
            Ok(())
        );
    }
}
