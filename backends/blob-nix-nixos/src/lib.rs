#![forbid(unsafe_code)]

use std::path::PathBuf;

use blob_core::{
    FeatureState, KernelPolicy, SystemArchitecture, SystemAuthorityClass, SystemCandidateAction,
    SystemCandidateId, SystemCandidateOperation, SystemChannel, SystemEffectClass, SystemFeatureId,
    SystemFeatureSelection, SystemOperationId, SystemOperationViolation, SystemSpec, SystemSpecId,
    SystemSpecViolation,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NixOsCandidateTarget {
    pub flake_path: PathBuf,
    pub configuration: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NixOperationPlanError {
    InvalidOperation(Vec<SystemOperationViolation>),
    EmptyFlakePath,
    InvalidConfigurationName,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NixCommandPlan {
    pub operation_id: SystemOperationId,
    pub candidate: SystemCandidateId,
    pub system_spec: SystemSpecId,
    pub action: SystemCandidateAction,
    pub effect_class: SystemEffectClass,
    pub authority: SystemAuthorityClass,
    pub program: String,
    pub args: Vec<String>,
    pub expected_effects: Vec<String>,
    pub rollback_semantics: String,
}

impl NixCommandPlan {
    pub fn changes_live_system(&self) -> bool {
        self.effect_class == SystemEffectClass::TemporaryLiveActivation
    }
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

    pub fn plan_operation(
        operation: &SystemCandidateOperation,
        target: &NixOsCandidateTarget,
    ) -> Result<NixCommandPlan, NixOperationPlanError> {
        operation
            .validate_policy()
            .map_err(NixOperationPlanError::InvalidOperation)?;
        validate_target(target)?;

        let flake_selector = format!(
            "{}#{}",
            target.flake_path.display(),
            target.configuration
        );

        let (program, args, expected_effects, rollback_semantics) = match operation.action {
            SystemCandidateAction::Materialize => (
                "nix".to_owned(),
                vec![
                    "build".into(),
                    "--no-link".into(),
                    "--print-out-paths".into(),
                    format!(
                        "{flake_selector}.config.system.build.toplevel"
                    ),
                ],
                vec![
                    "materialize immutable candidate closure in the Nix store".into(),
                    "do not activate the candidate on the running host".into(),
                ],
                "No live-system rollback is needed because the candidate is not activated.".into(),
            ),
            SystemCandidateAction::BuildIsolatedVm => (
                "nix".to_owned(),
                vec![
                    "build".into(),
                    "--no-link".into(),
                    "--print-out-paths".into(),
                    format!("{flake_selector}.config.system.build.vm"),
                ],
                vec![
                    "materialize an isolated QEMU VM candidate in the Nix store".into(),
                    "do not activate the candidate on the running host".into(),
                ],
                "Discard the VM artifact/cache reference; the running host is unchanged.".into(),
            ),
            SystemCandidateAction::PreviewActivation => (
                "nixos-rebuild".to_owned(),
                vec!["dry-activate".into(), "--flake".into(), flake_selector],
                vec![
                    "build the candidate and calculate live activation changes".into(),
                    "backend dry-activation hooks explicitly marked as supported may execute".into(),
                ],
                "No configuration switch is performed; dry-activation hooks must be captured as evidence.".into(),
            ),
            SystemCandidateAction::TestActivation => (
                "nixos-rebuild".to_owned(),
                vec!["test".into(), "--flake".into(), flake_selector],
                vec![
                    "build and temporarily activate the candidate on the running host".into(),
                    "do not make the candidate the boot-default generation".into(),
                ],
                "Reboot returns to the previous boot-default configuration; an explicit rollback path should also be recorded before execution.".into(),
            ),
        };

        Ok(NixCommandPlan {
            operation_id: operation.id.clone(),
            candidate: operation.candidate.clone(),
            system_spec: operation.system_spec.clone(),
            action: operation.action.clone(),
            effect_class: operation.effect_class.clone(),
            authority: operation.authority.clone(),
            program,
            args,
            expected_effects,
            rollback_semantics,
        })
    }
}

fn validate_target(target: &NixOsCandidateTarget) -> Result<(), NixOperationPlanError> {
    if target.flake_path.as_os_str().is_empty() {
        return Err(NixOperationPlanError::EmptyFlakePath);
    }

    if !valid_configuration_name(&target.configuration) {
        return Err(NixOperationPlanError::InvalidConfigurationName);
    }

    Ok(())
}

fn valid_configuration_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 63 {
        return false;
    }
    let bytes = name.as_bytes();
    if bytes.first() == Some(&b'-') || bytes.last() == Some(&b'-') {
        return false;
    }
    bytes
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
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

    fn target() -> NixOsCandidateTarget {
        NixOsCandidateTarget {
            flake_path: PathBuf::from("/var/lib/theblob/candidates/abc"),
            configuration: "blob-pilot".into(),
        }
    }

    fn operation(action: SystemCandidateAction) -> SystemCandidateOperation {
        SystemCandidateOperation::new(
            "op:one",
            "candidate:one",
            "system:linux-pilot",
            action,
        )
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

    #[test]
    fn materialize_plan_uses_nix_build_and_never_activates() {
        let plan = NixOsBackend::plan_operation(
            &operation(SystemCandidateAction::Materialize),
            &target(),
        )
        .expect("valid materialize plan");

        assert_eq!(plan.program, "nix");
        assert_eq!(plan.args[0], "build");
        assert!(plan.args.iter().all(|arg| arg != "switch" && arg != "boot"));
        assert!(!plan.changes_live_system());
    }

    #[test]
    fn vm_plan_targets_system_build_vm() {
        let plan = NixOsBackend::plan_operation(
            &operation(SystemCandidateAction::BuildIsolatedVm),
            &target(),
        )
        .expect("valid VM plan");

        assert_eq!(plan.program, "nix");
        assert!(plan
            .args
            .last()
            .expect("derivation selector")
            .ends_with(".config.system.build.vm"));
        assert!(!plan.changes_live_system());
    }

    #[test]
    fn dry_activate_is_admin_preview_not_pure_materialization() {
        let plan = NixOsBackend::plan_operation(
            &operation(SystemCandidateAction::PreviewActivation),
            &target(),
        )
        .expect("valid preview plan");

        assert_eq!(plan.program, "nixos-rebuild");
        assert_eq!(plan.args[0], "dry-activate");
        assert_eq!(plan.effect_class, SystemEffectClass::PreviewHooks);
        assert_eq!(plan.authority, SystemAuthorityClass::HostAdministrator);
        assert!(!plan.changes_live_system());
    }

    #[test]
    fn test_activation_is_explicit_temporary_live_change() {
        let plan = NixOsBackend::plan_operation(
            &operation(SystemCandidateAction::TestActivation),
            &target(),
        )
        .expect("valid test plan");

        assert_eq!(plan.program, "nixos-rebuild");
        assert_eq!(plan.args[0], "test");
        assert_eq!(plan.effect_class, SystemEffectClass::TemporaryLiveActivation);
        assert_eq!(plan.authority, SystemAuthorityClass::HostAdministrator);
        assert!(plan.changes_live_system());
    }

    #[test]
    fn forged_operation_policy_is_rejected_by_backend() {
        let mut forged = operation(SystemCandidateAction::TestActivation);
        forged.effect_class = SystemEffectClass::MaterializationOnly;
        forged.authority = SystemAuthorityClass::User;

        assert!(matches!(
            NixOsBackend::plan_operation(&forged, &target()),
            Err(NixOperationPlanError::InvalidOperation(_))
        ));
    }

    #[test]
    fn invalid_configuration_name_is_rejected() {
        let mut invalid = target();
        invalid.configuration = "blob pilot; switch".into();

        assert_eq!(
            NixOsBackend::plan_operation(
                &operation(SystemCandidateAction::Materialize),
                &invalid
            ),
            Err(NixOperationPlanError::InvalidConfigurationName)
        );
    }

    #[test]
    fn no_v0_1_action_can_generate_persistent_activation() {
        for action in [
            SystemCandidateAction::Materialize,
            SystemCandidateAction::PreviewActivation,
            SystemCandidateAction::TestActivation,
            SystemCandidateAction::BuildIsolatedVm,
        ] {
            let plan = NixOsBackend::plan_operation(&operation(action), &target())
                .expect("all v0.1 actions should plan");
            assert!(plan.args.iter().all(|arg| arg != "switch" && arg != "boot"));
        }
    }
}
