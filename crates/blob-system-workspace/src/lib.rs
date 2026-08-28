#![forbid(unsafe_code)]

use blob_core::{
    FeatureState, KernelPolicy, SemanticBuildProfile, SystemArchitecture, SystemChannel,
    SystemConstructionMode, SystemFeatureId, SystemFeatureSelection, SystemPriority,
    SystemProfileId, SystemSpec, SystemSpecId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemWorkspaceRequest {
    SetFeature {
        feature: SystemFeatureId,
        state: FeatureState,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemFeatureChange {
    pub feature: SystemFeatureId,
    pub from: Option<FeatureState>,
    pub to: FeatureState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemWorkspaceProposal {
    pub title: String,
    pub desired_outcome: String,
    pub baseline: SystemSpec,
    pub proposed: SystemSpec,
    pub changes: Vec<SystemFeatureChange>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemWorkspaceError {
    InvalidBaseline,
    InvalidProposal,
    NoChange,
}

impl SystemWorkspaceProposal {
    pub fn propose(
        baseline: &SystemSpec,
        request: SystemWorkspaceRequest,
        desired_outcome: impl Into<String>,
    ) -> Result<Self, SystemWorkspaceError> {
        baseline
            .validate()
            .map_err(|_| SystemWorkspaceError::InvalidBaseline)?;

        let mut proposed = baseline.clone();
        proposed.id = SystemSpecId::from(format!("{}:proposal", baseline.id));

        let change = match request {
            SystemWorkspaceRequest::SetFeature { feature, state } => {
                let existing = proposed
                    .profile
                    .features
                    .iter_mut()
                    .find(|selection| selection.feature == feature);

                let from = existing.as_ref().map(|selection| selection.state.clone());
                if from.as_ref() == Some(&state) {
                    return Err(SystemWorkspaceError::NoChange);
                }

                match existing {
                    Some(selection) => selection.state = state.clone(),
                    None => proposed.profile.features.push(SystemFeatureSelection {
                        feature: feature.clone(),
                        state: state.clone(),
                    }),
                }

                SystemFeatureChange {
                    feature,
                    from,
                    to: state,
                }
            }
        };

        proposed
            .validate()
            .map_err(|_| SystemWorkspaceError::InvalidProposal)?;

        Ok(Self {
            title: format!("Change {}", change.feature),
            desired_outcome: desired_outcome.into(),
            baseline: baseline.clone(),
            proposed,
            changes: vec![change],
        })
    }

    pub fn bluetooth_demo() -> Self {
        let baseline = demo_baseline_system_spec();
        Self::propose(
            &baseline,
            SystemWorkspaceRequest::SetFeature {
                feature: SystemFeatureId::from("bluetooth"),
                state: FeatureState::Enabled,
            },
            "Enable Bluetooth in an isolated NixOS demo candidate.",
        )
        .expect("the built-in Bluetooth demo proposal must be valid")
    }

    pub fn semantic_diff_lines(&self) -> Vec<String> {
        self.changes
            .iter()
            .map(|change| {
                format!(
                    "feature:{}: {} -> {}",
                    change.feature,
                    state_name(change.from.as_ref()),
                    state_name(Some(&change.to)),
                )
            })
            .collect()
    }
}

pub fn demo_baseline_system_spec() -> SystemSpec {
    SystemSpec {
        id: SystemSpecId::from("system:demo-workspace"),
        hostname: "blob-demo".into(),
        architecture: SystemArchitecture::X86_64,
        base_channel: SystemChannel::Stable,
        kernel_policy: KernelPolicy::DistributionDefault,
        profile: SemanticBuildProfile {
            id: SystemProfileId::from("profile:demo-balanced"),
            construction_mode: SystemConstructionMode::AiDesigned,
            priorities: vec![SystemPriority::Reliability, SystemPriority::Energy],
            features: vec![
                SystemFeatureSelection::disabled("bluetooth"),
                SystemFeatureSelection::enabled("pipewire"),
                SystemFeatureSelection::disabled("printing"),
            ],
        },
        experience_profile: None,
    }
}

fn state_name(state: Option<&FeatureState>) -> &'static str {
    match state {
        Some(FeatureState::Enabled) => "enabled",
        Some(FeatureState::Disabled) => "disabled",
        None => "unset",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bluetooth_demo_changes_only_the_semantic_feature_state() {
        let proposal = SystemWorkspaceProposal::bluetooth_demo();
        assert_eq!(proposal.changes.len(), 1);
        assert_eq!(proposal.changes[0].feature.as_str(), "bluetooth");
        assert_eq!(proposal.changes[0].from, Some(FeatureState::Disabled));
        assert_eq!(proposal.changes[0].to, FeatureState::Enabled);
        assert_eq!(proposal.baseline.hostname, proposal.proposed.hostname);
        assert_eq!(proposal.baseline.base_channel, proposal.proposed.base_channel);
        assert_eq!(
            proposal.semantic_diff_lines(),
            vec!["feature:bluetooth: disabled -> enabled"]
        );
    }

    #[test]
    fn requesting_current_state_is_not_a_fake_change() {
        let baseline = demo_baseline_system_spec();
        let error = SystemWorkspaceProposal::propose(
            &baseline,
            SystemWorkspaceRequest::SetFeature {
                feature: SystemFeatureId::from("bluetooth"),
                state: FeatureState::Disabled,
            },
            "Keep Bluetooth disabled",
        )
        .expect_err("no-op requests must be explicit");
        assert_eq!(error, SystemWorkspaceError::NoChange);
    }

    #[test]
    fn proposal_keeps_baseline_immutable() {
        let baseline = demo_baseline_system_spec();
        let proposal = SystemWorkspaceProposal::propose(
            &baseline,
            SystemWorkspaceRequest::SetFeature {
                feature: SystemFeatureId::from("ssh"),
                state: FeatureState::Enabled,
            },
            "Enable SSH",
        )
        .expect("valid proposal");
        assert!(baseline
            .profile
            .features
            .iter()
            .all(|selection| selection.feature.as_str() != "ssh"));
        assert!(proposal
            .proposed
            .profile
            .features
            .iter()
            .any(|selection| selection.feature.as_str() == "ssh" && selection.state == FeatureState::Enabled));
    }
}
