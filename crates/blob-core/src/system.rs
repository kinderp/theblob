use std::collections::BTreeSet;

use crate::ids::{ExperienceProfileId, SystemFeatureId, SystemProfileId, SystemSpecId};

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
}
