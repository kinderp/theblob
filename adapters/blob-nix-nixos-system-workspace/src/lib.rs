#![forbid(unsafe_code)]

use blob_nix_nixos::{NixBackendError, NixOsBackend, TranslationStep};
use blob_nix_nixos_candidate_producer::canonical_system_spec;
use blob_system_workspace::SystemWorkspaceProposal;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NixOsSystemWorkspacePreview {
    pub title: String,
    pub desired_outcome: String,
    pub semantic_diff: Vec<String>,
    pub baseline_module: String,
    pub proposed_module: String,
    pub proposed_canonical_system_spec: String,
    pub translation_trace: Vec<TranslationStep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NixOsSystemWorkspacePreviewError {
    BaselineTranslation(NixBackendError),
    ProposedTranslation(NixBackendError),
}

impl NixOsSystemWorkspacePreview {
    pub fn from_proposal(
        proposal: &SystemWorkspaceProposal,
    ) -> Result<Self, NixOsSystemWorkspacePreviewError> {
        let baseline = NixOsBackend::translate(&proposal.baseline)
            .map_err(NixOsSystemWorkspacePreviewError::BaselineTranslation)?;
        let proposed = NixOsBackend::translate(&proposal.proposed)
            .map_err(NixOsSystemWorkspacePreviewError::ProposedTranslation)?;

        Ok(Self {
            title: proposal.title.clone(),
            desired_outcome: proposal.desired_outcome.clone(),
            semantic_diff: proposal.semantic_diff_lines(),
            baseline_module: baseline.module_text,
            proposed_module: proposed.module_text,
            proposed_canonical_system_spec: canonical_system_spec(&proposal.proposed),
            translation_trace: proposed.trace,
        })
    }

    pub fn bluetooth_demo() -> Self {
        Self::from_proposal(&SystemWorkspaceProposal::bluetooth_demo())
            .expect("the built-in Bluetooth demo must be supported by the NixOS backend")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bluetooth_preview_keeps_native_translation_out_of_the_system_workspace_model() {
        let preview = NixOsSystemWorkspacePreview::bluetooth_demo();
        assert_eq!(
            preview.semantic_diff,
            vec!["feature:bluetooth: disabled -> enabled"]
        );
        assert!(preview
            .baseline_module
            .contains("hardware.bluetooth.enable = false;"));
        assert!(preview
            .proposed_module
            .contains("hardware.bluetooth.enable = true;"));
        assert!(preview
            .proposed_canonical_system_spec
            .starts_with("theblob-system-spec-v1\n"));
        assert!(preview.translation_trace.iter().any(|step| {
            step.semantic_source == "feature:bluetooth=enabled"
                && step.nix_target == "hardware.bluetooth.enable"
        }));
    }

    #[test]
    fn canonical_request_contains_no_native_nix_escape_hatch() {
        let preview = NixOsSystemWorkspacePreview::bluetooth_demo();
        assert!(!preview.proposed_canonical_system_spec.contains("hardware.bluetooth.enable"));
        assert!(!preview.proposed_canonical_system_spec.contains("raw-nix"));
        assert!(!preview.proposed_canonical_system_spec.contains("builtins."));
        assert!(!preview.proposed_canonical_system_spec.contains("--impure"));
    }
}
