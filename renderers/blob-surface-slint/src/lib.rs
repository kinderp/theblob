#![deny(unsafe_code)]

use blob_mvp::VerticalSliceOutcome;

slint::include_modules!();

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevelopmentSurfaceSnapshot {
    pub workspace_title: String,
    pub situation_summary: String,
    pub task_status: String,
    pub binding_summary: String,
    pub verifier_summary: String,
    pub execution_output: String,
    pub causal_summary: String,
}

impl DevelopmentSurfaceSnapshot {
    pub fn from_vertical_slice(outcome: &VerticalSliceOutcome) -> Self {
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

        Self {
            workspace_title: "Development · The Blob".into(),
            situation_summary: outcome.situation.summary.clone(),
            task_status: format!("{:?}", outcome.task.state),
            binding_summary: binding,
            verifier_summary: verifier,
            execution_output: execution,
            causal_summary,
        }
    }
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
    ui.run()
}
