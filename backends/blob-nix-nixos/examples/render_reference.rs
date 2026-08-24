use blob_core::{
    ExperienceProfileId, KernelPolicy, SemanticBuildProfile, SystemArchitecture,
    SystemConstructionMode, SystemFeatureSelection, SystemPriority, SystemProfileId, SystemSpec,
    SystemSpecId, SystemChannel,
};
use blob_nix_nixos::NixOsBackend;

fn main() {
    let spec = SystemSpec {
        id: SystemSpecId::from("system:linux-pilot"),
        hostname: "blob-pilot".into(),
        architecture: SystemArchitecture::X86_64,
        base_channel: SystemChannel::Stable,
        kernel_policy: KernelPolicy::LatestSupported,
        profile: SemanticBuildProfile {
            id: SystemProfileId::from("profile:development-balanced"),
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
    };

    let translation = NixOsBackend::translate(&spec).expect("reference SystemSpec must translate");
    print!("{}", translation.module_text);
}
