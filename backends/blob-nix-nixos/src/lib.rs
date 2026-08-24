#![forbid(unsafe_code)]

use blob_core::{
    FeatureState, KernelPolicy, SystemArchitecture, SystemChannel, SystemFeatureId,
    SystemFeatureSelection, SystemSpec, SystemSpecViolation,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationStep {
    pub semantic_source: String,
    pub nix_target: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NixTranslation {
    pub module_text: String,
    pub trace: Vec<TranslationStep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NixBackendError {
    InvalidSpec(Vec<SystemSpecViolation>),
    UnsupportedChannel(SystemChannel),
    UnsupportedFeature(SystemFeatureId),
}

pub struct NixOsBackend;

impl NixOsBackend {
    pub fn translate(spec: &SystemSpec) -> Result<NixTranslation, NixBackendError> {
        spec.validate().map_err(NixBackendError::InvalidSpec)?;

        if spec.base_channel != SystemChannel::Stable {
            return Err(NixBackendError::UnsupportedChannel(
                spec.base_channel.clone(),
            ));
        }

        let mut lines = Vec::new();
        let mut trace = Vec::new();

        lines.push("{ pkgs, ... }:".to_owned());
        lines.push("{".to_owned());
        lines.push(format!("  networking.hostName = \"{}\";", spec.hostname));
        trace.push(TranslationStep {
            semantic_source: "SystemSpec.hostname".into(),
            nix_target: "networking.hostName".into(),
        });

        let host_platform = match spec.architecture {
            SystemArchitecture::X86_64 => "x86_64-linux",
            SystemArchitecture::Aarch64 => "aarch64-linux",
        };
        lines.push(format!(
            "  nixpkgs.hostPlatform = \"{host_platform}\";"
        ));
        trace.push(TranslationStep {
            semantic_source: "SystemSpec.architecture".into(),
            nix_target: "nixpkgs.hostPlatform".into(),
        });

        match spec.kernel_policy {
            KernelPolicy::DistributionDefault => {
                trace.push(TranslationStep {
                    semantic_source: "SystemSpec.kernel_policy=distribution-default".into(),
                    nix_target: "backend default kernel selection".into(),
                });
            }
            KernelPolicy::LatestSupported => {
                lines.push("  boot.kernelPackages = pkgs.linuxPackages_latest;".into());
                trace.push(TranslationStep {
                    semantic_source: "SystemSpec.kernel_policy=latest-supported".into(),
                    nix_target: "boot.kernelPackages = pkgs.linuxPackages_latest".into(),
                });
            }
        }

        let mut features = spec.profile.features.clone();
        features.sort_by(|left, right| left.feature.as_str().cmp(right.feature.as_str()));
        for feature in &features {
            let (option, value) = translate_feature(feature)?;
            lines.push(format!("  {option} = {value};"));
            trace.push(TranslationStep {
                semantic_source: format!("feature:{}={}", feature.feature, state_name(&feature.state)),
                nix_target: option.into(),
            });
        }

        lines.push("}".to_owned());

        Ok(NixTranslation {
            module_text: lines.join("\n") + "\n",
            trace,
        })
    }
}

fn state_name(state: &FeatureState) -> &'static str {
    match state {
        FeatureState::Enabled => "enabled",
        FeatureState::Disabled => "disabled",
    }
}

fn bool_literal(state: &FeatureState) -> &'static str {
    match state {
        FeatureState::Enabled => "true",
        FeatureState::Disabled => "false",
    }
}

fn translate_feature(
    selection: &SystemFeatureSelection,
) -> Result<(&'static str, &'static str), NixBackendError> {
    let option = match selection.feature.as_str() {
        "bluetooth" => "hardware.bluetooth.enable",
        "containers" => "virtualisation.podman.enable",
        "flatpak" => "services.flatpak.enable",
        "hyprland" => "programs.hyprland.enable",
        "pipewire" => "services.pipewire.enable",
        "printing" => "services.printing.enable",
        "ssh" => "services.openssh.enable",
        _ => {
            return Err(NixBackendError::UnsupportedFeature(
                selection.feature.clone(),
            ));
        }
    };

    Ok((option, bool_literal(&selection.state)))
}

#[cfg(test)]
mod tests {
    use blob_core::{
        ExperienceProfileId, SemanticBuildProfile, SystemConstructionMode, SystemPriority,
        SystemProfileId, SystemSpecId,
    };

    use super::*;

    fn spec() -> SystemSpec {
        SystemSpec {
            id: SystemSpecId::from("system:linux-pilot"),
            hostname: "blob-pilot".into(),
            architecture: SystemArchitecture::X86_64,
            base_channel: SystemChannel::Stable,
            kernel_policy: KernelPolicy::LatestSupported,
            profile: SemanticBuildProfile {
                id: SystemProfileId::from("profile:dev"),
                construction_mode: SystemConstructionMode::AiDesigned,
                priorities: vec![SystemPriority::Reliability, SystemPriority::Energy],
                features: vec![
                    SystemFeatureSelection::enabled("pipewire"),
                    SystemFeatureSelection::disabled("printing"),
                    SystemFeatureSelection::enabled("bluetooth"),
                    SystemFeatureSelection::enabled("hyprland"),
                ],
            },
            experience_profile: Some(ExperienceProfileId::from("experience:hyprland")),
        }
    }

    #[test]
    fn renders_a_deterministic_nixos_module() {
        let output = NixOsBackend::translate(&spec()).expect("supported spec");
        assert_eq!(
            output.module_text,
            "{ pkgs, ... }:\n{\n  networking.hostName = \"blob-pilot\";\n  nixpkgs.hostPlatform = \"x86_64-linux\";\n  boot.kernelPackages = pkgs.linuxPackages_latest;\n  hardware.bluetooth.enable = true;\n  programs.hyprland.enable = true;\n  services.pipewire.enable = true;\n  services.printing.enable = false;\n}\n"
        );
    }

    #[test]
    fn trace_explains_semantic_to_nix_translation() {
        let output = NixOsBackend::translate(&spec()).expect("supported spec");
        assert!(output.trace.iter().any(|step| {
            step.semantic_source == "feature:bluetooth=enabled"
                && step.nix_target == "hardware.bluetooth.enable"
        }));
        assert!(output.trace.iter().any(|step| {
            step.semantic_source == "SystemSpec.kernel_policy=latest-supported"
                && step.nix_target.contains("linuxPackages_latest")
        }));
    }

    #[test]
    fn unsupported_feature_is_never_silently_ignored() {
        let mut candidate = spec();
        candidate
            .profile
            .features
            .push(SystemFeatureSelection::enabled("unknown-feature"));
        assert_eq!(
            NixOsBackend::translate(&candidate),
            Err(NixBackendError::UnsupportedFeature(SystemFeatureId::from(
                "unknown-feature"
            )))
        );
    }

    #[test]
    fn unsupported_channel_is_explicit() {
        let mut candidate = spec();
        candidate.base_channel = SystemChannel::Edge;
        assert_eq!(
            NixOsBackend::translate(&candidate),
            Err(NixBackendError::UnsupportedChannel(SystemChannel::Edge))
        );
    }

    #[test]
    fn invalid_spec_is_rejected_before_translation() {
        let mut candidate = spec();
        candidate.hostname = "Bad Host".into();
        assert!(matches!(
            NixOsBackend::translate(&candidate),
            Err(NixBackendError::InvalidSpec(_))
        ));
    }
}
