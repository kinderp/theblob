#![forbid(unsafe_code)]

use blob_core::{
    ExperienceProfileId, KernelPolicy, SemanticBuildProfile, SystemArchitecture, SystemChannel,
    SystemConstructionMode, SystemFeatureSelection, SystemPriority, SystemProfileId, SystemSpec,
    SystemSpecId,
};
use blob_nix_nixos_candidate_producer::canonical_system_spec;

fn main() {
    let spec = SystemSpec {
        id: SystemSpecId::from("system:manifest-producer-vm"),
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
    print!("{}", canonical_system_spec(&spec));
}
