use crate::{BlobshCompletionItem, BlobshDepth, BlobshDocKind, BlobshDocRef};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobshCompletionSet {
    pub subject: String,
    pub explanation: String,
    pub source: String,
    pub items: Vec<BlobshCompletionItem>,
}

impl BlobshCompletionSet {
    pub fn empty(depth: BlobshDepth) -> Self {
        Self {
            subject: format!("{} · no structured completion", depth.as_str()),
            explanation: "No deterministic completion grammar is available at this materialized level yet. Ask AI for contextual help or edit the representation directly.".into(),
            source: "Blobsh trace contract".into(),
            items: Vec::new(),
        }
    }
}

fn schema_doc(target: &str) -> BlobshDocRef {
    BlobshDocRef {
        kind: BlobshDocKind::BlobSchema,
        title: "Blobsh command schema".into(),
        target: target.into(),
    }
}

fn item(
    insert_text: &str,
    label: &str,
    explanation: &str,
    recommended: bool,
    doc_target: &str,
) -> BlobshCompletionItem {
    BlobshCompletionItem {
        insert_text: insert_text.into(),
        label: label.into(),
        explanation: explanation.into(),
        recommended,
        documentation: vec![schema_doc(doc_target)],
    }
}

fn starts_with_ci(value: &str, prefix: &str) -> bool {
    value.to_ascii_lowercase().starts_with(&prefix.to_ascii_lowercase())
}

/// Return deterministic, data-only completion for the command representation
/// currently being edited. P0 intentionally implements only the INTENT grammar
/// that the demo runtime can really process. Deeper grammars arrive with their
/// actual validators/materializers rather than being guessed by the UI.
pub fn complete(depth: BlobshDepth, raw: &str) -> BlobshCompletionSet {
    if depth != BlobshDepth::Intent {
        return BlobshCompletionSet::empty(depth);
    }

    let text = raw.trim_start();
    let normalized = text.to_ascii_lowercase();

    if normalized.is_empty() {
        return BlobshCompletionSet {
            subject: "INTENT · choose a command family".into(),
            explanation: "Start from a semantic namespace. Completion only exposes command families implemented by the current Blobsh demo runtime.".into(),
            source: "Blobsh P0 intent grammar · docs/BLOBSH-v0.1.md".into(),
            items: vec![
                item("workspace.", "workspace.", "Navigate or collapse semantic Workspaces.", true, "intent/workspace"),
                item("system.", "system.", "Inspect or propose semantic system changes.", false, "intent/system"),
                item("technician.", "technician.", "Ask the evidence-backed Technician for an explanation.", false, "intent/technician"),
            ],
        };
    }

    if normalized == "workspace" || normalized == "workspace." {
        return BlobshCompletionSet {
            subject: "INTENT · workspace namespace".into(),
            explanation: "Choose whether to open one Workspace or collapse the current Workspace expansion back to World mode.".into(),
            source: "Blobsh schema · workspace commands".into(),
            items: vec![
                item("workspace.open ", "open", "Open/focus a named semantic Workspace.", true, "intent/workspace/open"),
                item("workspace.collapse ", "collapse", "Collapse an expanded Workspace presentation.", false, "intent/workspace/collapse"),
            ],
        };
    }

    if starts_with_ci(&normalized, "workspace.o") && !normalized.starts_with("workspace.open ") {
        return BlobshCompletionSet {
            subject: "INTENT · workspace verb".into(),
            explanation: "`open` selects a Workspace without coupling the intent to a window or renderer.".into(),
            source: "Blobsh schema · workspace.open".into(),
            items: vec![item("workspace.open ", "open", "Open/focus a named semantic Workspace.", true, "intent/workspace/open")],
        };
    }

    if normalized == "workspace.open" || normalized.starts_with("workspace.open ") {
        let target_prefix = normalized.strip_prefix("workspace.open").unwrap_or("").trim();
        let targets = [
            ("romeo", "Development Workspace: editor, terminal, docs and tests."),
            ("docs", "Documentation/knowledge Workspace."),
            ("system", "System state, health and proposal Workspace."),
            ("notes", "Low-friction scratchpad and task Workspace."),
        ];
        let items = targets
            .into_iter()
            .filter(|(target, _)| target_prefix.is_empty() || target.starts_with(target_prefix))
            .map(|(target, explanation)| {
                item(
                    &format!("workspace.open {target}"),
                    target,
                    explanation,
                    target == "romeo",
                    &format!("intent/workspace/{target}"),
                )
            })
            .collect();
        return BlobshCompletionSet {
            subject: "INTENT · workspace.open target".into(),
            explanation: "Choose the persistent semantic Workspace to focus. This does not prescribe a particular window, device or renderer.".into(),
            source: "Blobsh schema · Workspace identities".into(),
            items,
        };
    }

    if normalized == "workspace.collapse" || normalized.starts_with("workspace.collapse ") {
        return BlobshCompletionSet {
            subject: "INTENT · workspace.collapse target".into(),
            explanation: "P0 supports collapsing all expanded Workspace presentations back to the World view.".into(),
            source: "Blobsh schema · workspace.collapse".into(),
            items: if "all".starts_with(normalized.strip_prefix("workspace.collapse").unwrap_or("").trim()) {
                vec![item("workspace.collapse all", "all", "Return to World mode while preserving Workspace state.", true, "intent/workspace/collapse")]
            } else {
                Vec::new()
            },
        };
    }

    if normalized == "system" || normalized == "system." {
        return BlobshCompletionSet {
            subject: "INTENT · system namespace".into(),
            explanation: "P0 currently exposes Bluetooth as the first real semantic System Workspace vertical slice.".into(),
            source: "Blobsh schema · System Workspace P0".into(),
            items: vec![item("system.bluetooth ", "bluetooth", "Inspect or propose Bluetooth state through SystemSpec.", true, "intent/system/bluetooth")],
        };
    }

    if starts_with_ci(&normalized, "system.b") && !normalized.starts_with("system.bluetooth ") {
        return BlobshCompletionSet {
            subject: "INTENT · system capability".into(),
            explanation: "Bluetooth is materialized semantically; the UI never emits a raw host command at this level.".into(),
            source: "System Workspace semantic schema".into(),
            items: vec![item("system.bluetooth ", "bluetooth", "Bluetooth desired-state capability.", true, "intent/system/bluetooth")],
        };
    }

    if normalized == "system.bluetooth" || normalized.starts_with("system.bluetooth ") {
        let action_prefix = normalized.strip_prefix("system.bluetooth").unwrap_or("").trim();
        let items = if "enable".starts_with(action_prefix) {
            vec![item("system.bluetooth enable", "enable", "Prepare the validated disabled → enabled semantic proposal. No activation occurs at INTENT.", true, "intent/system/bluetooth/enable")]
        } else {
            Vec::new()
        };
        return BlobshCompletionSet {
            subject: "INTENT · system.bluetooth action".into(),
            explanation: "Choose the semantic desired action. P0 currently supports only `enable`; unsupported actions are not suggested.".into(),
            source: "SystemWorkspaceProposal::bluetooth_demo".into(),
            items,
        };
    }

    if normalized == "technician" || normalized == "technician." {
        return BlobshCompletionSet {
            subject: "INTENT · technician namespace".into(),
            explanation: "The Technician explains current evidence without receiving execution authority.".into(),
            source: "Technician intent contract".into(),
            items: vec![item("technician.explain ", "explain", "Explain current semantic evidence and causal context.", true, "intent/technician/explain")],
        };
    }

    if normalized == "technician.explain" || normalized.starts_with("technician.explain ") {
        let target_prefix = normalized.strip_prefix("technician.explain").unwrap_or("").trim();
        return BlobshCompletionSet {
            subject: "INTENT · technician.explain target".into(),
            explanation: "P0 can explain the currently active semantic evidence.".into(),
            source: "TechnicianEvidenceContext".into(),
            items: if "current".starts_with(target_prefix) {
                vec![item("technician.explain current", "current", "Explain the active context using validated evidence.", true, "intent/technician/explain/current")]
            } else {
                Vec::new()
            },
        };
    }

    if starts_with_ci("workspace.", &normalized) || starts_with_ci("system.", &normalized) || starts_with_ci("technician.", &normalized) {
        let mut items = Vec::new();
        if "workspace.".starts_with(&normalized) {
            items.push(item("workspace.", "workspace.", "Workspace navigation commands.", true, "intent/workspace"));
        }
        if "system.".starts_with(&normalized) {
            items.push(item("system.", "system.", "Semantic system commands.", false, "intent/system"));
        }
        if "technician.".starts_with(&normalized) {
            items.push(item("technician.", "technician.", "Evidence-backed explanation commands.", false, "intent/technician"));
        }
        return BlobshCompletionSet {
            subject: "INTENT · complete namespace".into(),
            explanation: "Complete the semantic command namespace; no shell/native operation is selected yet.".into(),
            source: "Blobsh P0 intent grammar".into(),
            items,
        };
    }

    BlobshCompletionSet {
        subject: "INTENT · no deterministic match".into(),
        explanation: "This text is outside the current deterministic P0 grammar. Keep editing, choose a completion, or ask AI to translate the request without executing it.".into(),
        source: "Blobsh P0 intent grammar".into(),
        items: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inserts(set: BlobshCompletionSet) -> Vec<String> {
        set.items.into_iter().map(|item| item.insert_text).collect()
    }

    #[test]
    fn workspace_namespace_completes_verbs() {
        assert_eq!(
            inserts(complete(BlobshDepth::Intent, "workspace.")),
            vec!["workspace.open ", "workspace.collapse "]
        );
    }

    #[test]
    fn workspace_open_completes_targets_by_prefix() {
        assert_eq!(
            inserts(complete(BlobshDepth::Intent, "workspace.open s")),
            vec!["workspace.open system"]
        );
        assert_eq!(
            inserts(complete(BlobshDepth::Intent, "workspace.open ")),
            vec![
                "workspace.open romeo",
                "workspace.open docs",
                "workspace.open system",
                "workspace.open notes",
            ]
        );
    }

    #[test]
    fn bluetooth_namespace_never_invents_unsupported_actions() {
        assert_eq!(
            inserts(complete(BlobshDepth::Intent, "system.bluetooth ")),
            vec!["system.bluetooth enable"]
        );
        assert!(complete(BlobshDepth::Intent, "system.bluetooth disable").items.is_empty());
    }

    #[test]
    fn deeper_levels_have_no_fake_completion_grammar() {
        let set = complete(BlobshDepth::Spec, "feature:bluetooth");
        assert!(set.items.is_empty());
        assert_eq!(set.subject, "SPEC · no structured completion");
    }
}
