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

fn filtered_items(
    prefix: &str,
    candidates: &[(&str, &str, &str, bool, &str)],
) -> Vec<BlobshCompletionItem> {
    candidates
        .iter()
        .filter(|(insert_text, _, _, _, _)| insert_text.starts_with(prefix))
        .map(|(insert_text, label, explanation, recommended, doc_target)| {
            item(insert_text, label, explanation, *recommended, doc_target)
        })
        .collect()
}

fn set(
    subject: &str,
    explanation: &str,
    source: &str,
    items: Vec<BlobshCompletionItem>,
) -> BlobshCompletionSet {
    BlobshCompletionSet {
        subject: subject.into(),
        explanation: explanation.into(),
        source: source.into(),
        items,
    }
}

/// Return deterministic, data-only completion for the command representation
/// currently being edited. P0 intentionally implements only the INTENT grammar
/// that the demo runtime can really process. Deeper grammars arrive with their
/// actual validators/materializers rather than being guessed by the UI.
pub fn complete(depth: BlobshDepth, raw: &str) -> BlobshCompletionSet {
    if depth != BlobshDepth::Intent {
        return BlobshCompletionSet::empty(depth);
    }

    let normalized = raw.trim_start().to_ascii_lowercase();

    const ROOT: &[(&str, &str, &str, bool, &str)] = &[
        (
            "workspace.",
            "workspace.",
            "Navigate or collapse semantic Workspaces.",
            true,
            "intent/workspace",
        ),
        (
            "system.",
            "system.",
            "Inspect or propose semantic system changes.",
            false,
            "intent/system",
        ),
        (
            "technician.",
            "technician.",
            "Ask the evidence-backed Technician for an explanation.",
            false,
            "intent/technician",
        ),
        (
            "inspector.",
            "inspector.",
            "Inspect the current semantic evidence without changing state.",
            false,
            "intent/inspector",
        ),
    ];

    if normalized.is_empty() || !normalized.contains('.') {
        let items = if normalized.is_empty() {
            ROOT.iter()
                .map(|(a, b, c, d, e)| item(a, b, c, *d, e))
                .collect()
        } else {
            filtered_items(&normalized, ROOT)
        };
        return set(
            "INTENT · choose a command family",
            "Start from a semantic namespace. Completion only exposes command families implemented by the current Blobsh demo runtime.",
            "Blobsh P0 intent grammar · docs/BLOBSH-v0.1.md",
            items,
        );
    }

    const WORKSPACE_VERBS: &[(&str, &str, &str, bool, &str)] = &[
        (
            "workspace.open ",
            "open",
            "Open/focus a named semantic Workspace.",
            true,
            "intent/workspace/open",
        ),
        (
            "workspace.collapse ",
            "collapse",
            "Collapse an expanded Workspace presentation.",
            false,
            "intent/workspace/collapse",
        ),
    ];
    if normalized == "workspace." || (normalized.starts_with("workspace.")
        && !normalized.starts_with("workspace.open")
        && !normalized.starts_with("workspace.collapse"))
    {
        return set(
            "INTENT · workspace verb",
            "Choose whether to open one Workspace or collapse the current Workspace expansion back to World mode.",
            "Blobsh schema · workspace commands",
            filtered_items(&normalized, WORKSPACE_VERBS),
        );
    }

    if normalized == "workspace.open" || normalized.starts_with("workspace.open ") {
        let target_prefix = normalized
            .strip_prefix("workspace.open")
            .unwrap_or("")
            .trim();
        let targets = [
            (
                "romeo",
                "Development Workspace: editor, terminal, docs and tests.",
            ),
            ("docs", "Documentation/knowledge Workspace."),
            ("system", "System state, health and proposal Workspace."),
            ("notes", "Low-friction scratchpad and task Workspace."),
        ];
        let items = targets
            .into_iter()
            .filter(|(target, _)| target.starts_with(target_prefix))
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
        return set(
            "INTENT · workspace.open target",
            "Choose the persistent semantic Workspace to focus. This does not prescribe a particular window, device or renderer.",
            "Blobsh schema · Workspace identities",
            items,
        );
    }

    if normalized == "workspace.collapse" || normalized.starts_with("workspace.collapse ") {
        let target_prefix = normalized
            .strip_prefix("workspace.collapse")
            .unwrap_or("")
            .trim();
        let items = if "all".starts_with(target_prefix) {
            vec![item(
                "workspace.collapse all",
                "all",
                "Return to World mode while preserving Workspace state.",
                true,
                "intent/workspace/collapse",
            )]
        } else {
            Vec::new()
        };
        return set(
            "INTENT · workspace.collapse target",
            "P0 supports collapsing all expanded Workspace presentations back to the World view.",
            "Blobsh schema · workspace.collapse",
            items,
        );
    }

    const SYSTEM_CAPABILITIES: &[(&str, &str, &str, bool, &str)] = &[(
        "system.bluetooth ",
        "bluetooth",
        "Inspect or propose Bluetooth state through SystemSpec.",
        true,
        "intent/system/bluetooth",
    )];
    if normalized == "system." || (normalized.starts_with("system.")
        && !normalized.starts_with("system.bluetooth"))
    {
        return set(
            "INTENT · system capability",
            "P0 currently exposes Bluetooth as the first real semantic System Workspace vertical slice.",
            "Blobsh schema · System Workspace P0",
            filtered_items(&normalized, SYSTEM_CAPABILITIES),
        );
    }

    if normalized == "system.bluetooth" || normalized.starts_with("system.bluetooth ") {
        let action_prefix = normalized
            .strip_prefix("system.bluetooth")
            .unwrap_or("")
            .trim();
        let items = if "enable".starts_with(action_prefix) {
            vec![item(
                "system.bluetooth enable",
                "enable",
                "Prepare the validated disabled → enabled semantic proposal. No activation occurs at INTENT.",
                true,
                "intent/system/bluetooth/enable",
            )]
        } else {
            Vec::new()
        };
        return set(
            "INTENT · system.bluetooth action",
            "Choose the semantic desired action. P0 currently supports only `enable`; unsupported actions are not suggested.",
            "SystemWorkspaceProposal::bluetooth_demo",
            items,
        );
    }

    const TECHNICIAN_VERBS: &[(&str, &str, &str, bool, &str)] = &[(
        "technician.explain ",
        "explain",
        "Explain current semantic evidence and causal context.",
        true,
        "intent/technician/explain",
    )];
    if normalized == "technician." || (normalized.starts_with("technician.")
        && !normalized.starts_with("technician.explain"))
    {
        return set(
            "INTENT · technician verb",
            "The Technician explains current evidence without receiving execution authority.",
            "Technician intent contract",
            filtered_items(&normalized, TECHNICIAN_VERBS),
        );
    }

    if normalized == "technician.explain" || normalized.starts_with("technician.explain ") {
        let target_prefix = normalized
            .strip_prefix("technician.explain")
            .unwrap_or("")
            .trim();
        let items = if "current".starts_with(target_prefix) {
            vec![item(
                "technician.explain current",
                "current",
                "Explain the active context using validated evidence.",
                true,
                "intent/technician/explain/current",
            )]
        } else {
            Vec::new()
        };
        return set(
            "INTENT · technician.explain target",
            "P0 can explain the currently active semantic evidence.",
            "TechnicianEvidenceContext",
            items,
        );
    }

    const INSPECTOR_VERBS: &[(&str, &str, &str, bool, &str)] = &[(
        "inspector.show ",
        "show",
        "Open read-only semantic evidence details.",
        true,
        "intent/inspector/show",
    )];
    if normalized == "inspector." || (normalized.starts_with("inspector.")
        && !normalized.starts_with("inspector.show"))
    {
        return set(
            "INTENT · inspector verb",
            "Inspector commands expose evidence without changing system state.",
            "Blobsh schema · inspector",
            filtered_items(&normalized, INSPECTOR_VERBS),
        );
    }

    if normalized == "inspector.show" || normalized.starts_with("inspector.show ") {
        let target_prefix = normalized
            .strip_prefix("inspector.show")
            .unwrap_or("")
            .trim();
        let items = if "current".starts_with(target_prefix) {
            vec![item(
                "inspector.show current",
                "current",
                "Show the active semantic binding, verifier and causal evidence.",
                true,
                "intent/inspector/show/current",
            )]
        } else {
            Vec::new()
        };
        return set(
            "INTENT · inspector.show target",
            "Choose which evidence context to inspect. P0 exposes the current context.",
            "Blobsh schema · inspector.show",
            items,
        );
    }

    set(
        "INTENT · no deterministic match",
        "This text is outside the current deterministic P0 grammar. Keep editing or ask AI to translate the request without executing it.",
        "Blobsh P0 intent grammar",
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inserts(set: BlobshCompletionSet) -> Vec<String> {
        set.items
            .into_iter()
            .map(|item| item.insert_text)
            .collect()
    }

    #[test]
    fn workspace_namespace_completes_verbs_from_partial_prefixes() {
        assert_eq!(
            inserts(complete(BlobshDepth::Intent, "workspace.")),
            vec!["workspace.open ", "workspace.collapse "]
        );
        assert_eq!(
            inserts(complete(BlobshDepth::Intent, "workspace.c")),
            vec!["workspace.collapse "]
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
    fn root_prefixes_include_all_processable_intent_families() {
        assert_eq!(
            inserts(complete(BlobshDepth::Intent, "i")),
            vec!["inspector."]
        );
        assert_eq!(
            inserts(complete(BlobshDepth::Intent, "t")),
            vec!["technician."]
        );
    }

    #[test]
    fn bluetooth_namespace_never_invents_unsupported_actions() {
        assert_eq!(
            inserts(complete(BlobshDepth::Intent, "system.bluetooth ")),
            vec!["system.bluetooth enable"]
        );
        assert!(
            complete(BlobshDepth::Intent, "system.bluetooth disable")
                .items
                .is_empty()
        );
    }

    #[test]
    fn deeper_levels_have_no_fake_completion_grammar() {
        let set = complete(BlobshDepth::Spec, "feature:bluetooth");
        assert!(set.items.is_empty());
        assert_eq!(set.subject, "SPEC · no structured completion");
    }
}
