#![deny(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;

use blob_core::{FeatureState, SystemSpec};
use blob_mvp::VerticalSliceOutcome;
use blob_surface_app::character::{BlobAction, BlobBehaviorContext};
use blob_surface_app::shell::BlobShellState;
use blob_surface_app::{
    TechnicianEvidenceContext, TechnicianIntent, TechnicianProjection, TechnicianProjectionError,
};
use blob_system_workspace::{demo_baseline_system_spec, SystemWorkspaceProposal};
use blobsh_core::grammar::{complete as complete_blobsh, BlobshCompletionSet};
use blobsh_core::{
    BlobshCommandLayer, BlobshCommandStatus, BlobshCommandTrace, BlobshDepth, BlobshProvenance,
};

slint::include_modules!();

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechnicianProjectionSet {
    pub explain: TechnicianProjection,
    pub teach: TechnicianProjection,
    pub prepare: TechnicianProjection,
}

impl TechnicianProjectionSet {
    fn for_intent(&self, intent: TechnicianIntent) -> &TechnicianProjection {
        match intent {
            TechnicianIntent::ExplainCurrent => &self.explain,
            TechnicianIntent::TeachCurrent => &self.teach,
            TechnicianIntent::PrepareNextStep => &self.prepare,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevelopmentSurfaceSnapshot {
    pub workspace_title: String,
    pub situation_summary: String,
    pub task_status: String,
    pub binding_summary: String,
    pub verifier_summary: String,
    pub execution_output: String,
    pub causal_summary: String,
    pub technician: TechnicianProjectionSet,
}

impl DevelopmentSurfaceSnapshot {
    pub fn from_vertical_slice(
        outcome: &VerticalSliceOutcome,
    ) -> Result<Self, TechnicianProjectionError> {
        let binding = outcome
            .plan
            .resolved_capabilities
            .first()
            .map(|resolved| {
                format!(
                    "{} → {} on {}",
                    resolved.capability, resolved.implementation, resolved.node
                )
            })
            .unwrap_or_else(|| "no binding".into());

        let verifier = if outcome.plan.trace.verifier_notes.is_empty() {
            "No verifier evidence is available.".into()
        } else {
            outcome.plan.trace.verifier_notes.join(" · ")
        };

        let execution = format!(
            "{:?} · exit {:?} · {}",
            outcome.execution.status,
            outcome.execution.exit_code,
            outcome.execution.stdout.trim()
        );

        let causal_summary = outcome
            .causal_records
            .iter()
            .enumerate()
            .map(|(index, record)| format!("{}. {}", index + 1, record.summary))
            .collect::<Vec<_>>()
            .join("\n");

        let context = TechnicianEvidenceContext {
            situation: &outcome.situation,
            task: &outcome.task,
            plan: &outcome.plan,
            causal_records: &outcome.causal_records,
        };
        let technician = TechnicianProjectionSet {
            explain: context.project(TechnicianIntent::ExplainCurrent)?,
            teach: context.project(TechnicianIntent::TeachCurrent)?,
            prepare: context.project(TechnicianIntent::PrepareNextStep)?,
        };

        Ok(Self {
            workspace_title: "Development · The Blob".into(),
            situation_summary: outcome.situation.summary.clone(),
            task_status: format!("{:?}", outcome.task.state),
            binding_summary: binding,
            verifier_summary: verifier,
            execution_output: execution,
            causal_summary,
            technician,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemWorkspaceSurfaceSnapshot {
    pub title: String,
    pub mode: String,
    pub bluetooth_enabled: bool,
    pub bluetooth_label: String,
    pub pipewire_label: String,
    pub printing_label: String,
}

impl SystemWorkspaceSurfaceSnapshot {
    pub fn demo() -> Self {
        let baseline = demo_baseline_system_spec();
        Self {
            title: "Personal Computer".into(),
            mode: "AI Designed".into(),
            bluetooth_enabled: feature_enabled(&baseline, "bluetooth"),
            bluetooth_label: feature_label(&baseline, "bluetooth", "Bluetooth"),
            pipewire_label: feature_label(&baseline, "pipewire", "PipeWire"),
            printing_label: feature_label(&baseline, "printing", "Printing"),
        }
    }
}

fn feature_state<'a>(spec: &'a SystemSpec, feature: &str) -> Option<&'a FeatureState> {
    spec.profile
        .features
        .iter()
        .find(|selection| selection.feature.as_str() == feature)
        .map(|selection| &selection.state)
}

fn feature_enabled(spec: &SystemSpec, feature: &str) -> bool {
    matches!(feature_state(spec, feature), Some(FeatureState::Enabled))
}

fn feature_label(spec: &SystemSpec, feature: &str, display_name: &str) -> String {
    let state = match feature_state(spec, feature) {
        Some(FeatureState::Enabled) => "enabled",
        Some(FeatureState::Disabled) => "disabled",
        None => "unset",
    };
    format!("{display_name} {state}")
}

fn projection_detail(projection: &TechnicianProjection) -> String {
    let mut lines = projection
        .details
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>();
    if let Some(next) = projection.next_steps.first() {
        lines.push(format!("Next: {next}"));
    }
    if let Some(safety) = projection.safety_notes.first() {
        lines.push(format!("Safety: {safety}"));
    }
    lines.join("\n")
}

fn show_projection(ui: &BlobShell, projection: &TechnicianProjection) {
    ui.set_overlay_open(true);
    ui.set_overlay_eyebrow("TECHNICIAN · EVIDENCE".into());
    ui.set_overlay_title(projection.title.clone().into());
    ui.set_overlay_body(projection.summary.clone().into());
    ui.set_overlay_detail(projection_detail(projection).into());
    ui.set_overlay_status(
        format!(
            "required autonomy {:?} · no execution authority granted",
            projection.required_autonomy
        )
        .into(),
    );
}

fn show_details(ui: &BlobShell, snapshot: &DevelopmentSurfaceSnapshot) {
    ui.set_overlay_open(true);
    ui.set_overlay_eyebrow("WHY? / DETAILS".into());
    ui.set_overlay_title("Current semantic evidence".into());
    ui.set_overlay_body(snapshot.binding_summary.clone().into());
    ui.set_overlay_detail(
        format!(
            "Verifier: {}\nExecution: {}\n\nCausal history:\n{}",
            snapshot.verifier_summary, snapshot.execution_output, snapshot.causal_summary
        )
        .into(),
    );
    ui.set_overlay_status("read-only inspection · renderer has no execution authority".into());
}

fn apply_shell_state(ui: &BlobShell, shell: &BlobShellState) {
    ui.set_focused_workspace(shell.focused_workspace_index() as i32);
    ui.set_expanded_workspace(
        shell
            .expanded_workspace_index()
            .map(|index| index as i32)
            .unwrap_or(-1),
    );
}

fn demo_character_actions(task_status: &str, system_problem: bool) -> [BlobAction; 4] {
    let romeo = BlobBehaviorContext {
        task_running: task_status != "Completed",
        ..BlobBehaviorContext::calm()
    }
    .resolve();
    let system = BlobBehaviorContext {
        problem: system_problem,
        ..BlobBehaviorContext::calm()
    }
    .resolve();

    [romeo, BlobAction::Idle, system, BlobAction::Idle]
}

fn apply_character_actions(ui: &BlobShell, actions: [BlobAction; 4]) {
    ui.set_romeo_action(actions[0].as_str().into());
    ui.set_docs_action(actions[1].as_str().into());
    ui.set_system_action(actions[2].as_str().into());
    ui.set_notes_action(actions[3].as_str().into());
}

fn apply_bluetooth_proposal(ui: &BlobShell, proposal: &SystemWorkspaceProposal) {
    ui.set_system_proposal_pending(true);
    ui.set_system_proposal_title(proposal.title.clone().into());
    ui.set_system_proposal_summary(proposal.desired_outcome.clone().into());
    ui.set_system_proposal_diff(proposal.semantic_diff_lines().join(" · ").into());
    ui.set_system_action(BlobAction::Warning.as_str().into());
    ui.set_overlay_open(true);
    ui.set_overlay_eyebrow("SYSTEM · PROPOSAL".into());
    ui.set_overlay_title("Bluetooth change proposed".into());
    ui.set_overlay_body(
        "The control created a validated semantic SystemSpec proposal. Bluetooth is still off in the baseline because no backend or authority step has run."
            .into(),
    );
    ui.set_overlay_detail(
        format!(
            "Semantic diff: {}\nNext boundary: deterministic backend translation and candidate verification.",
            proposal.semantic_diff_lines().join(" · ")
        )
        .into(),
    );
    ui.set_overlay_status(
        "proposal only · no raw Nix emitted by UI · no activation authority granted".into(),
    );
}

fn clear_system_proposal(ui: &BlobShell) {
    ui.set_system_proposal_pending(false);
    ui.set_system_proposal_title("".into());
    ui.set_system_proposal_summary("".into());
    ui.set_system_proposal_diff("".into());
    ui.set_system_action(BlobAction::Idle.as_str().into());
}

fn canonical_intent(raw: &str) -> (String, bool) {
    match raw.trim().to_lowercase().as_str() {
        "romeo" | "dev" => ("workspace.open romeo".into(), true),
        "docs" => ("workspace.open docs".into(), true),
        "system" => ("workspace.open system".into(), true),
        "notes" => ("workspace.open notes".into(), true),
        "bluetooth" | "enable bluetooth" | "attiva bluetooth" => {
            ("system.bluetooth enable".into(), true)
        }
        "why" | "why?" | "explain" | "spiega" => {
            ("technician.explain current".into(), true)
        }
        "details" | "dettagli" => ("inspector.show current".into(), true),
        "home" | "blob" => ("workspace.collapse all".into(), true),
        "" => (String::new(), false),
        other => (format!("intent.unresolved {other}"), false),
    }
}

fn sync_copilot(ui: &BlobShell, set: BlobshCompletionSet) {
    ui.set_copilot_title(set.subject.into());
    ui.set_copilot_body(set.explanation.into());
    ui.set_copilot_source(set.source.into());

    let mut items = set.items.into_iter();
    ui.set_completion_1(
        items
            .next()
            .map(|item| item.insert_text)
            .unwrap_or_default()
            .into(),
    );
    ui.set_completion_2(
        items
            .next()
            .map(|item| item.insert_text)
            .unwrap_or_default()
            .into(),
    );
    ui.set_completion_3(
        items
            .next()
            .map(|item| item.insert_text)
            .unwrap_or_default()
            .into(),
    );
}

fn refresh_copilot(ui: &BlobShell, depth: BlobshDepth, text: &str) {
    sync_copilot(ui, complete_blobsh(depth, text));
}

fn sync_blobsh_trace(ui: &BlobShell, trace: &BlobshCommandTrace) {
    let layer = trace.active_layer();
    ui.set_command_text(layer.text.clone().into());
    ui.set_command_level(trace.depth_indicator().into());
    ui.set_command_status(trace.status().as_str().into());
    ui.set_command_can_shallower(trace.can_select_shallower());
    ui.set_command_can_deeper(trace.can_select_deeper());
    refresh_copilot(ui, trace.active_depth(), &layer.text);
}

fn preview_command_edit(
    ui: &BlobShell,
    trace_slot: &Rc<RefCell<Option<BlobshCommandTrace>>>,
    raw: &str,
) {
    // Editing is a draft operation. Do not trim, normalize or write the draft
    // back into the materialized trace on every keystroke: doing so destroys
    // meaningful trailing whitespace while the user is still composing a
    // command (for example `workspace.open ` before the target is typed).
    // The draft is committed to the INTENT layer only when Enter processes it.
    let slot = trace_slot.borrow();
    let depth = slot
        .as_ref()
        .map(BlobshCommandTrace::active_depth)
        .unwrap_or(BlobshDepth::Intent);

    refresh_copilot(ui, depth, raw);

    if let Some(trace) = slot.as_ref() {
        ui.set_command_level(trace.depth_indicator().into());
        ui.set_command_can_shallower(trace.can_select_shallower());
        ui.set_command_can_deeper(trace.can_select_deeper());
    } else {
        ui.set_command_level("INTENT · draft".into());
        ui.set_command_can_shallower(false);
        ui.set_command_can_deeper(false);
    }

    ui.set_command_status("editing".into());
    ui.set_copilot_open(true);
}

fn select_completion(
    ui: &BlobShell,
    trace_slot: &Rc<RefCell<Option<BlobshCommandTrace>>>,
    replacement: &str,
) {
    ui.set_command_text(replacement.into());
    preview_command_edit(ui, trace_slot, replacement);
    ui.set_prompt_status("completion inserted · caret moved to end · review/edit · Enter to process".into());
}

fn ask_ai_about_command(
    ui: &BlobShell,
    trace_slot: &Rc<RefCell<Option<BlobshCommandTrace>>>,
    raw: &str,
) {
    let label = trace_slot
        .borrow()
        .as_ref()
        .map(BlobshCommandTrace::depth_indicator)
        .unwrap_or_else(|| "INTENT · draft".into());
    ui.set_prompt_text(
        format!("Aiutami a capire o modificare {label}: {raw}").into(),
    );
    ui.set_prompt_status(
        "AI assistance scoped to the current representation · edit the question, then Enter".into(),
    );
}

fn prepare_prompt_trace(
    ui: &BlobShell,
    trace_slot: &Rc<RefCell<Option<BlobshCommandTrace>>>,
    raw: &str,
) {
    if raw.trim().is_empty() {
        ui.set_prompt_status("ask The Blob · then inspect/edit below".into());
        return;
    }

    let Ok(mut trace) = BlobshCommandTrace::from_user_input(raw) else {
        ui.set_prompt_status("empty prompt".into());
        return;
    };
    let (intent, recognized) = canonical_intent(raw);
    let _ = trace.append_layer(BlobshCommandLayer::new(
        BlobshDepth::Intent,
        intent,
        true,
        BlobshProvenance::AiProposed,
    ));
    trace.set_status(if recognized {
        BlobshCommandStatus::Preview
    } else {
        BlobshCommandStatus::Invalid
    });

    sync_blobsh_trace(ui, &trace);
    *trace_slot.borrow_mut() = Some(trace);
    ui.set_prompt_text("".into());
    ui.set_prompt_status(if recognized {
        "AI translated the request · ↑ returns to USER · inspect/edit INTENT · Enter to process".into()
    } else {
        "P0 translator cannot resolve this request yet · edit the BLOB command directly".into()
    });
}

fn ensure_expert_trace(
    trace_slot: &Rc<RefCell<Option<BlobshCommandTrace>>>,
    raw: &str,
) -> bool {
    if trace_slot.borrow().is_some() {
        return true;
    }
    let Ok(trace) = BlobshCommandTrace::from_direct_intent(raw.trim()) else {
        return false;
    };
    *trace_slot.borrow_mut() = Some(trace);
    true
}

fn execute_blob_command(
    ui: &BlobShell,
    shell: &Rc<RefCell<BlobShellState>>,
    technician: &TechnicianProjectionSet,
    snapshot: &DevelopmentSurfaceSnapshot,
    trace_slot: &Rc<RefCell<Option<BlobshCommandTrace>>>,
    raw: &str,
) {
    if raw.trim().is_empty() {
        ui.set_prompt_status("BLOB command is empty".into());
        return;
    }
    if !ensure_expert_trace(trace_slot, raw) {
        ui.set_prompt_status("cannot create an expert INTENT from an empty command".into());
        return;
    }

    let mut slot = trace_slot.borrow_mut();
    let trace = slot.as_mut().expect("expert trace was ensured above");

    if trace.active_depth() != BlobshDepth::Intent {
        ui.set_prompt_status(
            "processing is anchored at INTENT in P0 · Alt+↑ or [↑] returns toward INTENT".into(),
        );
        return;
    }

    if trace.active_layer().text != raw.trim() {
        let _ = trace.edit_layer(BlobshDepth::Intent, raw.trim());
    }

    let command = raw.trim().to_lowercase();
    match command.as_str() {
        "workspace.open romeo" => {
            shell.borrow_mut().toggle_single_workspace(0);
            apply_shell_state(ui, &shell.borrow());
            trace.set_status(BlobshCommandStatus::Ready);
            ui.set_prompt_status("Romeo Workspace intent processed".into());
        }
        "workspace.open docs" => {
            shell.borrow_mut().toggle_single_workspace(1);
            apply_shell_state(ui, &shell.borrow());
            trace.set_status(BlobshCommandStatus::Ready);
            ui.set_prompt_status("Docs Workspace intent processed".into());
        }
        "workspace.open system" => {
            shell.borrow_mut().toggle_single_workspace(2);
            apply_shell_state(ui, &shell.borrow());
            trace.set_status(BlobshCommandStatus::Ready);
            ui.set_prompt_status("System Workspace intent processed".into());
        }
        "workspace.open notes" => {
            shell.borrow_mut().toggle_single_workspace(3);
            apply_shell_state(ui, &shell.borrow());
            trace.set_status(BlobshCommandStatus::Ready);
            ui.set_prompt_status("Notes Workspace intent processed".into());
        }
        "system.bluetooth enable" => {
            shell.borrow_mut().toggle_single_workspace(2);
            apply_shell_state(ui, &shell.borrow());
            let proposal = SystemWorkspaceProposal::bluetooth_demo();
            apply_bluetooth_proposal(ui, &proposal);

            let _ = trace.append_layer(BlobshCommandLayer::new(
                BlobshDepth::Plan,
                "target=local · proposal-only · authority=user",
                true,
                BlobshProvenance::Derived,
            ));
            let _ = trace.append_layer(BlobshCommandLayer::new(
                BlobshDepth::Spec,
                proposal.semantic_diff_lines().join(" · "),
                true,
                BlobshProvenance::Derived,
            ));
            trace.set_status(BlobshCommandStatus::ApprovalRequired);
            let _ = trace.select_depth(BlobshDepth::Intent);
            ui.set_prompt_status(
                "Bluetooth materialized PLAN + SPEC · stay on INTENT, then Alt+↓ / [↓] to drill down".into(),
            );
        }
        "technician.explain current" => {
            show_projection(ui, technician.for_intent(TechnicianIntent::ExplainCurrent));
            trace.set_status(BlobshCommandStatus::Ready);
            ui.set_prompt_status("Technician explanation opened".into());
        }
        "inspector.show current" => {
            show_details(ui, snapshot);
            trace.set_status(BlobshCommandStatus::Ready);
            ui.set_prompt_status("Semantic evidence inspector opened".into());
        }
        "workspace.collapse all" => {
            shell.borrow_mut().collapse_all();
            apply_shell_state(ui, &shell.borrow());
            trace.set_status(BlobshCommandStatus::Ready);
            ui.set_prompt_status("World mode restored".into());
        }
        _ => {
            trace.set_status(BlobshCommandStatus::Invalid);
            ui.set_prompt_status(
                "unknown P0 BLOB command · use completion, edit INTENT or ask AI".into(),
            );
        }
    }

    sync_blobsh_trace(ui, trace);
}

fn move_command_depth(
    ui: &BlobShell,
    trace_slot: &Rc<RefCell<Option<BlobshCommandTrace>>>,
    deeper: bool,
) {
    let mut slot = trace_slot.borrow_mut();
    let Some(trace) = slot.as_mut() else {
        ui.set_prompt_status("no materialized command trace yet".into());
        return;
    };

    let target = if deeper {
        trace.active_depth().deeper()
    } else {
        trace.active_depth().shallower()
    };
    let Some(target) = target else {
        ui.set_prompt_status("already at the end of this trace".into());
        return;
    };

    if trace.select_depth(target).is_ok() {
        sync_blobsh_trace(ui, trace);
        ui.set_prompt_status(format!("Depth Inspector · {}", target.as_str()).into());
    } else {
        sync_blobsh_trace(ui, trace);
        ui.set_prompt_status(
            format!(
                "{} is not materialized yet · The Blob never invents a lower-level command",
                target.as_str()
            )
            .into(),
        );
    }
}

pub fn show(snapshot: &DevelopmentSurfaceSnapshot) -> Result<(), slint::PlatformError> {
    let ui = BlobShell::new()?;
    ui.set_situation_summary(snapshot.situation_summary.clone().into());
    ui.set_task_status(snapshot.task_status.clone().into());
    ui.set_binding_summary(snapshot.binding_summary.clone().into());
    ui.set_verifier_summary(snapshot.verifier_summary.clone().into());
    ui.set_execution_output(snapshot.execution_output.clone().into());
    apply_character_actions(&ui, demo_character_actions(&snapshot.task_status, false));
    refresh_copilot(&ui, BlobshDepth::Intent, "");

    let system = SystemWorkspaceSurfaceSnapshot::demo();
    ui.set_bluetooth_enabled(system.bluetooth_enabled);
    ui.set_bluetooth_label(system.bluetooth_label.clone().into());
    ui.set_pipewire_label(system.pipewire_label.clone().into());
    ui.set_printing_label(system.printing_label.clone().into());

    let shell = Rc::new(RefCell::new(BlobShellState::demo_local("surface-host:macos-p0")));
    let technician = Rc::new(snapshot.technician.clone());
    let snapshot = Rc::new(snapshot.clone());
    let blobsh_trace = Rc::new(RefCell::new(None::<BlobshCommandTrace>));
    apply_shell_state(&ui, &shell.borrow());

    {
        let weak = ui.as_weak();
        let shell = Rc::clone(&shell);
        ui.on_activate_workspace(move |index| {
            if let Some(ui) = weak.upgrade() {
                if let Ok(index) = usize::try_from(index) {
                    shell.borrow_mut().toggle_single_workspace(index);
                    apply_shell_state(&ui, &shell.borrow());
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let shell = Rc::clone(&shell);
        ui.on_collapse_workspaces(move || {
            shell.borrow_mut().collapse_all();
            if let Some(ui) = weak.upgrade() {
                apply_shell_state(&ui, &shell.borrow());
            }
        });
    }
    {
        let weak = ui.as_weak();
        let technician = Rc::clone(&technician);
        ui.on_explain_current(move || {
            if let Some(ui) = weak.upgrade() {
                show_projection(&ui, technician.for_intent(TechnicianIntent::ExplainCurrent));
            }
        });
    }
    {
        let weak = ui.as_weak();
        let snapshot = Rc::clone(&snapshot);
        ui.on_show_details(move || {
            if let Some(ui) = weak.upgrade() {
                show_details(&ui, &snapshot);
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_close_overlay(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_overlay_open(false);
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_system_propose_bluetooth(move || {
            let proposal = SystemWorkspaceProposal::bluetooth_demo();
            if let Some(ui) = weak.upgrade() {
                apply_bluetooth_proposal(&ui, &proposal);
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_system_clear_proposal(move || {
            if let Some(ui) = weak.upgrade() {
                clear_system_proposal(&ui);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let blobsh_trace = Rc::clone(&blobsh_trace);
        ui.on_submit_prompt(move |prompt| {
            if let Some(ui) = weak.upgrade() {
                prepare_prompt_trace(&ui, &blobsh_trace, prompt.as_str());
            }
        });
    }
    {
        let weak = ui.as_weak();
        let shell = Rc::clone(&shell);
        let technician = Rc::clone(&technician);
        let snapshot = Rc::clone(&snapshot);
        let blobsh_trace = Rc::clone(&blobsh_trace);
        ui.on_submit_command(move |command| {
            if let Some(ui) = weak.upgrade() {
                execute_blob_command(
                    &ui,
                    &shell,
                    &technician,
                    &snapshot,
                    &blobsh_trace,
                    command.as_str(),
                );
            }
        });
    }
    {
        let weak = ui.as_weak();
        let blobsh_trace = Rc::clone(&blobsh_trace);
        ui.on_command_edited(move |command| {
            if let Some(ui) = weak.upgrade() {
                preview_command_edit(&ui, &blobsh_trace, command.as_str());
            }
        });
    }
    {
        let weak = ui.as_weak();
        let blobsh_trace = Rc::clone(&blobsh_trace);
        ui.on_completion_selected(move |completion| {
            if let Some(ui) = weak.upgrade() {
                select_completion(&ui, &blobsh_trace, completion.as_str());
            }
        });
    }
    {
        let weak = ui.as_weak();
        let blobsh_trace = Rc::clone(&blobsh_trace);
        ui.on_ask_ai_about_command(move |command| {
            if let Some(ui) = weak.upgrade() {
                ask_ai_about_command(&ui, &blobsh_trace, command.as_str());
            }
        });
    }
    {
        let weak = ui.as_weak();
        let blobsh_trace = Rc::clone(&blobsh_trace);
        ui.on_command_shallower(move || {
            if let Some(ui) = weak.upgrade() {
                move_command_depth(&ui, &blobsh_trace, false);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let blobsh_trace = Rc::clone(&blobsh_trace);
        ui.on_command_deeper(move || {
            if let Some(ui) = weak.upgrade() {
                move_command_depth(&ui, &blobsh_trace, true);
            }
        });
    }

    ui.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_surface_reads_the_semantic_demo_baseline() {
        let snapshot = SystemWorkspaceSurfaceSnapshot::demo();
        assert!(!snapshot.bluetooth_enabled);
        assert_eq!(snapshot.bluetooth_label, "Bluetooth disabled");
        assert_eq!(snapshot.pipewire_label, "PipeWire enabled");
        assert_eq!(snapshot.printing_label, "Printing disabled");
    }

    #[test]
    fn bluetooth_surface_action_is_a_semantic_proposal_not_local_state_mutation() {
        let baseline = SystemWorkspaceSurfaceSnapshot::demo();
        let proposal = SystemWorkspaceProposal::bluetooth_demo();

        assert!(!baseline.bluetooth_enabled);
        assert_eq!(
            proposal.semantic_diff_lines(),
            vec!["feature:bluetooth: disabled -> enabled"]
        );
    }

    #[test]
    fn shell_demo_exposes_four_renderer_neutral_workspaces() {
        let shell = BlobShellState::demo_local("host:test");
        assert_eq!(shell.workspaces.len(), 4);
        assert_eq!(shell.workspaces[0].title, "Romeo");
        assert_eq!(shell.workspaces[0].surface_instances.len(), 4);
    }

    #[test]
    fn semantic_behavior_resolver_drives_demo_character_actions() {
        assert_eq!(
            demo_character_actions("Running", false),
            [
                BlobAction::Busy,
                BlobAction::Idle,
                BlobAction::Idle,
                BlobAction::Idle,
            ]
        );
        assert_eq!(
            demo_character_actions("Completed", true),
            [
                BlobAction::Idle,
                BlobAction::Idle,
                BlobAction::Warning,
                BlobAction::Idle,
            ]
        );
    }

    #[test]
    fn ai_prompt_translation_produces_a_separate_editable_intent() {
        assert_eq!(
            canonical_intent("attiva bluetooth"),
            ("system.bluetooth enable".into(), true)
        );
        assert_eq!(
            canonical_intent("Romeo"),
            ("workspace.open romeo".into(), true)
        );
    }

    #[test]
    fn unknown_prompt_is_exposed_instead_of_silently_executed() {
        assert_eq!(
            canonical_intent("fai una cosa nuova"),
            ("intent.unresolved fai una cosa nuova".into(), false)
        );
    }

    #[test]
    fn renderer_uses_the_same_core_completion_grammar_as_future_frontends() {
        let set = complete_blobsh(BlobshDepth::Intent, "workspace.open s");
        assert_eq!(set.items.len(), 1);
        assert_eq!(set.items[0].insert_text, "workspace.open system");
    }
}
