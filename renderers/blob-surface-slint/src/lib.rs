#![deny(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;

use blob_core::{FeatureState, SystemSpec};
use blob_mvp::VerticalSliceOutcome;
use blob_surface_app::{
    PrimarySurface, SurfaceApplication, SurfaceEffect, SurfaceIntent, TechnicianEvidenceContext,
    TechnicianIntent, TechnicianProjection, TechnicianProjectionError,
};
use blob_system_workspace::{demo_baseline_system_spec, SystemWorkspaceProposal};

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

fn page_index(page: PrimarySurface) -> i32 {
    match page {
        PrimarySurface::Now => 0,
        PrimarySurface::CurrentWorkspace => 1,
        PrimarySurface::History => 2,
        PrimarySurface::Fabric => 3,
        PrimarySurface::Inspector => 4,
    }
}

fn projection_detail(projection: &TechnicianProjection) -> String {
    let mut lines = projection
        .details
        .iter()
        .take(2)
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

fn apply_effect(ui: &BlobShell, effect: SurfaceEffect, technician: &TechnicianProjectionSet) {
    match effect {
        SurfaceEffect::PageChanged(page) => ui.set_active_page(page_index(page)),
        SurfaceEffect::TechnicianIntentRecorded(intent) => {
            let projection = technician.for_intent(intent);
            ui.set_technician_title(projection.title.clone().into());
            ui.set_technician_body(projection.summary.clone().into());
            ui.set_technician_detail(projection_detail(projection).into());
            ui.set_technician_status(
                format!(
                    "Evidence-backed · required autonomy {:?} · no execution authority granted",
                    projection.required_autonomy
                )
                .into(),
            );
        }
    }
}

fn apply_bluetooth_proposal(ui: &BlobShell, proposal: &SystemWorkspaceProposal) {
    ui.set_system_proposal_pending(true);
    ui.set_system_proposal_title(proposal.title.clone().into());
    ui.set_system_proposal_summary(proposal.desired_outcome.clone().into());
    ui.set_system_proposal_diff(proposal.semantic_diff_lines().join(" · ").into());
    ui.set_technician_title("Bluetooth change proposed".into());
    ui.set_technician_body(
        "The switch created a validated semantic SystemSpec proposal. Bluetooth is still off in the baseline because no backend or authority step has run."
            .into(),
    );
    ui.set_technician_detail(
        format!(
            "Semantic diff: {}\nNext boundary: deterministic backend translation and candidate verification.",
            proposal.semantic_diff_lines().join(" · ")
        )
        .into(),
    );
    ui.set_technician_status(
        "Proposal only · no raw Nix emitted by the UI · no activation authority granted".into(),
    );
}

pub fn show(snapshot: &DevelopmentSurfaceSnapshot) -> Result<(), slint::PlatformError> {
    let ui = BlobShell::new()?;
    ui.set_workspace_title(snapshot.workspace_title.clone().into());
    ui.set_situation_summary(snapshot.situation_summary.clone().into());
    ui.set_task_status(snapshot.task_status.clone().into());
    ui.set_binding_summary(snapshot.binding_summary.clone().into());
    ui.set_verifier_summary(snapshot.verifier_summary.clone().into());
    ui.set_execution_output(snapshot.execution_output.clone().into());
    ui.set_causal_summary(snapshot.causal_summary.clone().into());

    let system = SystemWorkspaceSurfaceSnapshot::demo();
    ui.set_system_workspace_title(system.title.clone().into());
    ui.set_system_mode(system.mode.clone().into());
    ui.set_bluetooth_enabled(system.bluetooth_enabled);
    ui.set_bluetooth_label(system.bluetooth_label.clone().into());
    ui.set_pipewire_label(system.pipewire_label.clone().into());
    ui.set_printing_label(system.printing_label.clone().into());

    let app = Rc::new(RefCell::new(SurfaceApplication::default()));
    let technician = Rc::new(snapshot.technician.clone());
    ui.set_active_page(page_index(app.borrow().current()));

    {
        let weak = ui.as_weak();
        let app = Rc::clone(&app);
        let technician = Rc::clone(&technician);
        ui.on_navigate_now(move || {
            let effect = app
                .borrow_mut()
                .apply(SurfaceIntent::Navigate(PrimarySurface::Now));
            if let Some(ui) = weak.upgrade() {
                apply_effect(&ui, effect, &technician);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let app = Rc::clone(&app);
        let technician = Rc::clone(&technician);
        ui.on_navigate_workspace(move || {
            let effect = app
                .borrow_mut()
                .apply(SurfaceIntent::Navigate(PrimarySurface::CurrentWorkspace));
            if let Some(ui) = weak.upgrade() {
                apply_effect(&ui, effect, &technician);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let app = Rc::clone(&app);
        let technician = Rc::clone(&technician);
        ui.on_navigate_history(move || {
            let effect = app
                .borrow_mut()
                .apply(SurfaceIntent::Navigate(PrimarySurface::History));
            if let Some(ui) = weak.upgrade() {
                apply_effect(&ui, effect, &technician);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let app = Rc::clone(&app);
        let technician = Rc::clone(&technician);
        ui.on_navigate_fabric(move || {
            let effect = app
                .borrow_mut()
                .apply(SurfaceIntent::Navigate(PrimarySurface::Fabric));
            if let Some(ui) = weak.upgrade() {
                apply_effect(&ui, effect, &technician);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let app = Rc::clone(&app);
        let technician = Rc::clone(&technician);
        ui.on_open_inspector(move || {
            let effect = app.borrow_mut().apply(SurfaceIntent::OpenInspector);
            if let Some(ui) = weak.upgrade() {
                apply_effect(&ui, effect, &technician);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let app = Rc::clone(&app);
        let technician = Rc::clone(&technician);
        ui.on_close_inspector(move || {
            let effect = app.borrow_mut().apply(SurfaceIntent::CloseInspector);
            if let Some(ui) = weak.upgrade() {
                apply_effect(&ui, effect, &technician);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let app = Rc::clone(&app);
        let technician = Rc::clone(&technician);
        ui.on_technician_explain(move || {
            let effect = app.borrow_mut().apply(SurfaceIntent::Technician(
                TechnicianIntent::ExplainCurrent,
            ));
            if let Some(ui) = weak.upgrade() {
                apply_effect(&ui, effect, &technician);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let app = Rc::clone(&app);
        let technician = Rc::clone(&technician);
        ui.on_technician_teach(move || {
            let effect = app.borrow_mut().apply(SurfaceIntent::Technician(
                TechnicianIntent::TeachCurrent,
            ));
            if let Some(ui) = weak.upgrade() {
                apply_effect(&ui, effect, &technician);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let app = Rc::clone(&app);
        let technician = Rc::clone(&technician);
        ui.on_technician_prepare(move || {
            let effect = app.borrow_mut().apply(SurfaceIntent::Technician(
                TechnicianIntent::PrepareNextStep,
            ));
            if let Some(ui) = weak.upgrade() {
                apply_effect(&ui, effect, &technician);
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
                ui.set_system_proposal_pending(false);
                ui.set_system_proposal_title("".into());
                ui.set_system_proposal_summary("".into());
                ui.set_system_proposal_diff("".into());
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
}
