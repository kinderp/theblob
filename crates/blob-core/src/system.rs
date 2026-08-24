use std::collections::BTreeSet;

use crate::ids::{
    ExperienceProfileId, SystemCandidateId, SystemFeatureId, SystemOperationId, SystemProfileId,
    SystemSpecId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemConstructionMode {
    Ready,
    AiDesigned,
    Expert,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemArchitecture {
    X86_64,
    Aarch64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemChannel {
    Stable,
    Testing,
    Edge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KernelPolicy {
    DistributionDefault,
    LatestSupported,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SystemPriority {
    Reliability,
    Security,
    Latency,
    Energy,
    Memory,
    BuildTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeatureState {
    Enabled,
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemFeatureSelection {
    pub feature: SystemFeatureId,
    pub state: FeatureState,
}

impl SystemFeatureSelection {
    pub fn enabled(feature: impl Into<SystemFeatureId>) -> Self {
        Self {
            feature: feature.into(),
            state: FeatureState::Enabled,
        }
    }

    pub fn disabled(feature: impl Into<SystemFeatureId>) -> Self {
        Self {
            feature: feature.into(),
            state: FeatureState::Disabled,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticBuildProfile {
    pub id: SystemProfileId,
    pub construction_mode: SystemConstructionMode,
    pub priorities: Vec<SystemPriority>,
    pub features: Vec<SystemFeatureSelection>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemSpec {
    pub id: SystemSpecId,
    pub hostname: String,
    pub architecture: SystemArchitecture,
    pub base_channel: SystemChannel,
    pub kernel_policy: KernelPolicy,
    pub profile: SemanticBuildProfile,
    pub experience_profile: Option<ExperienceProfileId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemSpecViolation {
    InvalidHostname,
    DuplicateFeature(SystemFeatureId),
    DuplicatePriority(SystemPriority),
}

impl SystemSpec {
    pub fn validate(&self) -> Result<(), Vec<SystemSpecViolation>> {
        let mut violations = Vec::new();

        if !valid_hostname(&self.hostname) {
            violations.push(SystemSpecViolation::InvalidHostname);
        }

        let mut feature_ids = BTreeSet::new();
        for selection in &self.profile.features {
            if !feature_ids.insert(selection.feature.as_str().to_owned()) {
                violations.push(SystemSpecViolation::DuplicateFeature(
                    selection.feature.clone(),
                ));
            }
        }

        let mut priorities = BTreeSet::new();
        for priority in &self.profile.priorities {
            if !priorities.insert(priority.clone()) {
                violations.push(SystemSpecViolation::DuplicatePriority(priority.clone()));
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
pub enum SystemCandidateAction {
    /// Build/materialize the candidate without activating it on the live host.
    Materialize,
    /// Preview activation changes. A backend may execute explicitly declared
    /// dry-activation hooks, so this is not equivalent to a pure build.
    PreviewActivation,
    /// Temporarily activate the candidate on the live host without making it
    /// the boot default.
    TestActivation,
    /// Build an isolated virtual-machine representation of the candidate.
    BuildIsolatedVm,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemEffectClass {
    /// May write immutable build artifacts/caches but does not alter the
    /// currently running system configuration.
    MaterializationOnly,
    /// Does not switch the running system, but may invoke backend-specific
    /// dry-activation hooks that are explicitly marked safe for preview.
    PreviewHooks,
    /// Changes the running system temporarily; reboot/rollback semantics are
    /// expected to restore the previous boot-default state.
    TemporaryLiveActivation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemAuthorityClass {
    /// Normal user authority is sufficient for the operation itself.
    User,
    /// Host-administrator authority is required before execution.
    HostAdministrator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemCandidateOperation {
    pub id: SystemOperationId,
    pub candidate: SystemCandidateId,
    pub system_spec: SystemSpecId,
    pub action: SystemCandidateAction,
    pub effect_class: SystemEffectClass,
    pub authority: SystemAuthorityClass,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemOperationViolation {
    EffectClassMismatch {
        expected: SystemEffectClass,
        actual: SystemEffectClass,
    },
    AuthorityMismatch {
        expected: SystemAuthorityClass,
        actual: SystemAuthorityClass,
    },
}

impl SystemCandidateOperation {
    pub fn new(
        id: impl Into<SystemOperationId>,
        candidate: impl Into<SystemCandidateId>,
        system_spec: impl Into<SystemSpecId>,
        action: SystemCandidateAction,
    ) -> Self {
        let (effect_class, authority) = action_policy(&action);
        Self {
            id: id.into(),
            candidate: candidate.into(),
            system_spec: system_spec.into(),
            action,
            effect_class,
            authority,
        }
    }

    pub fn validate_policy(&self) -> Result<(), Vec<SystemOperationViolation>> {
        let (expected_effect, expected_authority) = action_policy(&self.action);
        let mut violations = Vec::new();

        if self.effect_class != expected_effect {
            violations.push(SystemOperationViolation::EffectClassMismatch {
                expected: expected_effect,
                actual: self.effect_class.clone(),
            });
        }

        if self.authority != expected_authority {
            violations.push(SystemOperationViolation::AuthorityMismatch {
                expected: expected_authority,
                actual: self.authority.clone(),
            });
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }

    pub fn changes_live_system(&self) -> bool {
        self.effect_class == SystemEffectClass::TemporaryLiveActivation
    }
}

fn action_policy(action: &SystemCandidateAction) -> (SystemEffectClass, SystemAuthorityClass) {
    match action {
        SystemCandidateAction::Materialize | SystemCandidateAction::BuildIsolatedVm => {
            (SystemEffectClass::MaterializationOnly, SystemAuthorityClass::User)
        }
        SystemCandidateAction::PreviewActivation => (
            SystemEffectClass::PreviewHooks,
            SystemAuthorityClass::HostAdministrator,
        ),
        SystemCandidateAction::TestActivation => (
            SystemEffectClass::TemporaryLiveActivation,
            SystemAuthorityClass::HostAdministrator,
        ),
    }
}

fn valid_hostname(hostname: &str) -> bool {
    if hostname.is_empty() || hostname.len() > 63 {
        return false;
    }

    let bytes = hostname.as_bytes();
    if bytes.first() == Some(&b'-') || bytes.last() == Some(&b'-') {
        return false;
    }

    bytes
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_spec() -> SystemSpec {
        SystemSpec {
            id: SystemSpecId::from("system:pilot"),
            hostname: "blob-pilot".into(),
            architecture: SystemArchitecture::X86_64,
            base_channel: SystemChannel::Stable,
            kernel_policy: KernelPolicy::DistributionDefault,
            profile: SemanticBuildProfile {
                id: SystemProfileId::from("profile:development-balanced"),
                construction_mode: SystemConstructionMode::AiDesigned,
                priorities: vec![SystemPriority::Reliability, SystemPriority::Energy],
                features: vec![
                    SystemFeatureSelection::enabled("bluetooth"),
                    SystemFeatureSelection::enabled("pipewire"),
                    SystemFeatureSelection::disabled("printing"),
                ],
            },
            experience_profile: Some(ExperienceProfileId::from("experience:hyprland")),
        }
    }

    #[test]
    fn valid_system_spec_is_accepted() {
        assert_eq!(valid_spec().validate(), Ok(()));
    }

    #[test]
    fn invalid_hostname_is_rejected() {
        let mut spec = valid_spec();
        spec.hostname = "Blob Pilot".into();
        assert!(matches!(
            spec.validate(),
            Err(violations) if violations.contains(&SystemSpecViolation::InvalidHostname)
        ));
    }

    #[test]
    fn duplicate_feature_is_rejected() {
        let mut spec = valid_spec();
        spec.profile
            .features
            .push(SystemFeatureSelection::enabled("bluetooth"));
        assert!(matches!(
            spec.validate(),
            Err(violations) if violations.contains(&SystemSpecViolation::DuplicateFeature(
                SystemFeatureId::from("bluetooth")
            ))
        ));
    }

    #[test]
    fn duplicate_priority_is_rejected() {
        let mut spec = valid_spec();
        spec.profile.priorities.push(SystemPriority::Energy);
        assert!(matches!(
            spec.validate(),
            Err(violations) if violations.contains(&SystemSpecViolation::DuplicatePriority(
                SystemPriority::Energy
            ))
        ));
    }

    #[test]
    fn materialize_is_non_live_user_operation() {
        let operation = SystemCandidateOperation::new(
            "op:build",
            "candidate:one",
            "system:pilot",
            SystemCandidateAction::Materialize,
        );
        assert_eq!(operation.effect_class, SystemEffectClass::MaterializationOnly);
        assert_eq!(operation.authority, SystemAuthorityClass::User);
        assert!(!operation.changes_live_system());
        assert_eq!(operation.validate_policy(), Ok(()));
    }

    #[test]
    fn preview_activation_is_not_mislabeled_as_pure_build() {
        let operation = SystemCandidateOperation::new(
            "op:preview",
            "candidate:one",
            "system:pilot",
            SystemCandidateAction::PreviewActivation,
        );
        assert_eq!(operation.effect_class, SystemEffectClass::PreviewHooks);
        assert_eq!(operation.authority, SystemAuthorityClass::HostAdministrator);
        assert!(!operation.changes_live_system());
        assert_eq!(operation.validate_policy(), Ok(()));
    }

    #[test]
    fn test_activation_is_explicit_live_mutation() {
        let operation = SystemCandidateOperation::new(
            "op:test",
            "candidate:one",
            "system:pilot",
            SystemCandidateAction::TestActivation,
        );
        assert_eq!(
            operation.effect_class,
            SystemEffectClass::TemporaryLiveActivation
        );
        assert_eq!(operation.authority, SystemAuthorityClass::HostAdministrator);
        assert!(operation.changes_live_system());
        assert_eq!(operation.validate_policy(), Ok(()));
    }

    #[test]
    fn vm_build_is_non_live_user_operation() {
        let operation = SystemCandidateOperation::new(
            "op:vm",
            "candidate:one",
            "system:pilot",
            SystemCandidateAction::BuildIsolatedVm,
        );
        assert_eq!(operation.effect_class, SystemEffectClass::MaterializationOnly);
        assert_eq!(operation.authority, SystemAuthorityClass::User);
        assert!(!operation.changes_live_system());
        assert_eq!(operation.validate_policy(), Ok(()));
    }

    #[test]
    fn forged_operation_policy_is_rejected() {
        let mut operation = SystemCandidateOperation::new(
            "op:test",
            "candidate:one",
            "system:pilot",
            SystemCandidateAction::TestActivation,
        );
        operation.effect_class = SystemEffectClass::MaterializationOnly;
        operation.authority = SystemAuthorityClass::User;

        assert!(matches!(
            operation.validate_policy(),
            Err(violations)
                if violations.iter().any(|violation| matches!(
                    violation,
                    SystemOperationViolation::EffectClassMismatch { .. }
                ))
                && violations.iter().any(|violation| matches!(
                    violation,
                    SystemOperationViolation::AuthorityMismatch { .. }
                ))
        ));
    }
}
