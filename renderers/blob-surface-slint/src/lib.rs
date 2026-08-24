#![forbid(unsafe_code)]

use blob_mvp::VerticalSliceOutcome;

slint::slint! {
    export component DevelopmentSurface inherits Window {
        in property <string> workspace-title;
        in property <string> situation-summary;
        in property <string> task-status;
        in property <string> binding-summary;
        in property <string> verifier-summary;
        in property <string> execution-output;
        in property <string> causal-summary;

        title: "The Blob — Development Workspace";
        width: 1100px;
        height: 720px;
        background: #0b0d12;

        VerticalLayout {
            padding: 24px;
            spacing: 14px;

            Text {
                text: "THE BLOB";
                color: #f4f6fb;
                font-size: 28px;
                font-weight: 700;
            }

            Text {
                text: root.workspace-title;
                color: #9aa7bd;
                font-size: 17px;
            }

            Rectangle {
                height: 92px;
                background: #151923;
                border-radius: 12px;
                border-width: 1px;
                border-color: #252b39;

                VerticalLayout {
                    padding: 14px;
                    spacing: 7px;
                    Text {
                        text: "SITUATION";
                        color: #7f8da6;
                        font-size: 12px;
                        font-weight: 700;
                    }
                    Text {
                        text: root.situation-summary;
                        color: #eef2f8;
                        font-size: 16px;
                    }
                }
            }

            HorizontalLayout {
                spacing: 14px;
                vertical-stretch: 1;

                Rectangle {
                    horizontal-stretch: 1;
                    background: #151923;
                    border-radius: 12px;
                    border-width: 1px;
                    border-color: #252b39;

                    VerticalLayout {
                        padding: 16px;
                        spacing: 10px;
                        Text { text: "TASK"; color: #7f8da6; font-size: 12px; font-weight: 700; }
                        Text { text: root.task-status; color: #7ee787; font-size: 20px; font-weight: 700; }
                        Text { text: "BINDING"; color: #7f8da6; font-size: 12px; font-weight: 700; }
                        Text { text: root.binding-summary; color: #eef2f8; font-size: 15px; }
                        Text { text: "VERIFIER"; color: #7f8da6; font-size: 12px; font-weight: 700; }
                        Text { text: root.verifier-summary; color: #c6d1e3; font-size: 14px; }
                    }
                }

                Rectangle {
                    horizontal-stretch: 1;
                    background: #111722;
                    border-radius: 12px;
                    border-width: 1px;
                    border-color: #233147;

                    VerticalLayout {
                        padding: 16px;
                        spacing: 10px;
                        Text { text: "EXECUTION RESULT"; color: #7f8da6; font-size: 12px; font-weight: 700; }
                        Text { text: root.execution-output; color: #d7e4f7; font-size: 18px; }
                        Text { text: "CAUSAL TRACE"; color: #7f8da6; font-size: 12px; font-weight: 700; }
                        Text {
                            text: root.causal-summary;
                            color: #aebbd0;
                            font-size: 13px;
                            wrap: word-wrap;
                        }
                    }
                }
            }

            Text {
                text: "Blob Native · semantic state, not screenshots";
                color: #65738b;
                font-size: 12px;
                horizontal-alignment: right;
            }
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
            "no verifier evidence".into()
        } else {
            outcome.plan.trace.verifier_notes.join(" · ")
        };

        let execution = format!(
            "{:?} · exit {:?}\n{}",
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
            workspace_title: "Development Workspace / MVP-0".into(),
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
    let ui = DevelopmentSurface::new()?;
    ui.set_workspace_title(snapshot.workspace_title.clone().into());
    ui.set_situation_summary(snapshot.situation_summary.clone().into());
    ui.set_task_status(snapshot.task_status.clone().into());
    ui.set_binding_summary(snapshot.binding_summary.clone().into());
    ui.set_verifier_summary(snapshot.verifier_summary.clone().into());
    ui.set_execution_output(snapshot.execution_output.clone().into());
    ui.set_causal_summary(snapshot.causal_summary.clone().into());
    ui.run()
}
