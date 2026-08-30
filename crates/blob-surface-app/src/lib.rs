#![forbid(unsafe_code)]

pub mod character;
pub mod shell;

use blob_core::{BindingPlan, CausalRecord, Situation, Task, TaskState, TechnicianAutonomy};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimarySurface {
    Now,
    CurrentWorkspace,
    History,
    Fabric,
    Inspector,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TechnicianIntent {
    ExplainCurrent,
    TeachCurrent,
    PrepareNextStep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceIntent {
    Navigate(PrimarySurface),
    OpenInspector,
    CloseInspector,
    Technician(TechnicianIntent),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceEffect {
    PageChanged(PrimarySurface),
    TechnicianIntentRecorded(TechnicianIntent),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechnicianProjection {
    pub title: String,
    pub summary: String,
    pub evidence: Vec<String>,
    pub details: Vec<String>,
    pub next_steps: Vec<String>,
    pub safety_notes: Vec<String>,
    pub required_autonomy: TechnicianAutonomy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TechnicianProjectionError {
    BindingDoesNotBelongToTaskRequirement,
}

pub struct TechnicianEvidenceContext<'a> {
    pub situation: &'a Situation,
    pub task: &'a Task,
    pub plan: &'a BindingPlan,
    pub causal_records: &'a [CausalRecord],
}

impl TechnicianEvidenceContext<'_> {
    pub fn project(
        &self,
        intent: TechnicianIntent,
    ) -> Result<TechnicianProjection, TechnicianProjectionError> {
        if !self
            .task
            .requirement_graphs
            .iter()
            .any(|graph| graph == &self.plan.graph)
        {
            return Err(TechnicianProjectionError::BindingDoesNotBelongToTaskRequirement);
        }

        let mut evidence = self
            .situation
            .evidence_event_ids
            .iter()
            .map(|event| format!("Evidence event: {event}"))
            .collect::<Vec<_>>();
        evidence.extend(
            self.situation
                .derived_facts
                .iter()
                .map(|(key, value)| format!("{key}={value}")),
        );
        evidence.extend(
            self.situation
                .deterministic_rule_provenance
                .iter()
                .map(|rule| format!("Rule: {rule}")),
        );

        let mut binding = self
            .plan
            .resolved_capabilities
            .iter()
            .map(|resolved| {
                format!(
                    "{} used implementation {} on node {}",
                    resolved.capability, resolved.implementation, resolved.node
                )
            })
            .collect::<Vec<_>>();
        binding.extend(
            self.plan
                .trace
                .verifier_notes
                .iter()
                .map(|note| format!("Verifier: {note}")),
        );

        let causal_steps = self
            .causal_records
            .iter()
            .map(|record| format!("{} — {}", record.actor, record.summary))
            .collect::<Vec<_>>();

        let read_only = vec![
            "This projection is read-only and does not authorize or execute a system change."
                .into(),
            "The explanation is derived from structured local evidence; no AI model is used in this demo slice."
                .into(),
        ];

        match intent {
            TechnicianIntent::ExplainCurrent => {
                let mut details = binding;
                details.extend(causal_steps);
                Ok(TechnicianProjection {
                    title: "What just happened?".into(),
                    summary: format!(
                        "{} The resulting task is {:?}.",
                        self.situation.summary, self.task.state
                    ),
                    evidence,
                    details,
                    next_steps: vec![
                        "Open Why? / Inspector for exact binding, verifier and causal evidence."
                            .into(),
                    ],
                    safety_notes: read_only,
                    required_autonomy: TechnicianAutonomy::Observe,
                })
            }
            TechnicianIntent::TeachCurrent => {
                let mut details = vec![
                    "1. Alfred correlated an Event into the current Situation.".into(),
                    "2. The Situation caused a Task with a semantic capability requirement.".into(),
                    "3. The resolver selected an implementation and node; that choice was not authority."
                        .into(),
                    "4. An independent verifier rechecked the concrete BindingPlan before execution."
                        .into(),
                    "5. Execution produced structured result state and causal history.".into(),
                ];
                details.extend(binding);
                Ok(TechnicianProjection {
                    title: "How The Blob reached this result".into(),
                    summary: "The important distinction is Situation → Task → Capability → Binding → independent verification → result. The Workspace never had to name an executable directly."
                        .into(),
                    evidence,
                    details,
                    next_steps: vec![
                        "Use Inspector to follow the same chain using the exact IDs and verifier evidence."
                            .into(),
                    ],
                    safety_notes: read_only,
                    required_autonomy: TechnicianAutonomy::Observe,
                })
            }
            TechnicianIntent::PrepareNextStep => {
                let (summary, mut next_steps) = if matches!(self.task.state, TaskState::Completed) {
                    (
                        "The current task completed successfully. The Blob will not invent a system change just because preparation was requested; a desired outcome is required first."
                            .to_owned(),
                        vec![
                            "State the desired outcome, for example a new feature, optimization or experiment."
                                .into(),
                            "The Technician can then produce a semantic proposal rather than a native command."
                                .into(),
                        ],
                    )
                } else {
                    (
                        format!(
                            "The current task is {:?}. A bounded next-step proposal may be useful, but this projection does not execute it.",
                            self.task.state
                        ),
                        vec![
                            "Use the current evidence to define the smallest reversible preparation proposal."
                                .into(),
                        ],
                    )
                };
                next_steps.push(
                    "Any future preparation must still cross policy, deterministic verification and the existing authority boundary before execution."
                        .into(),
                );
                let mut safety_notes = read_only;
                safety_notes.push(
                    "Prepare is an intent and autonomy level, not permission for privileged execution."
                        .into(),
                );
                Ok(TechnicianProjection {
                    title: "Preparation preview".into(),
                    summary,
                    evidence,
                    details: binding,
                    next_steps,
                    safety_notes,
                    required_autonomy: TechnicianAutonomy::Prepare,
                })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceApplication {
    current: PrimarySurface,
    inspector_return: PrimarySurface,
    last_technician_intent: Option<TechnicianIntent>,
}

impl Default for SurfaceApplication {
    fn default() -> Self {
        Self {
            current: PrimarySurface::Now,
            inspector_return: PrimarySurface::Now,
            last_technician_intent: None,
        }
    }
}

impl SurfaceApplication {
    pub fn current(&self) -> PrimarySurface {
        self.current
    }

    pub fn last_technician_intent(&self) -> Option<TechnicianIntent> {
        self.last_technician_intent
    }

    pub fn apply(&mut self, intent: SurfaceIntent) -> SurfaceEffect {
        match intent {
            SurfaceIntent::Navigate(page) => {
                self.current = page;
                SurfaceEffect::PageChanged(page)
            }
            SurfaceIntent::OpenInspector => {
                if self.current != PrimarySurface::Inspector {
                    self.inspector_return = self.current;
                }
                self.current = PrimarySurface::Inspector;
                SurfaceEffect::PageChanged(PrimarySurface::Inspector)
            }
            SurfaceIntent::CloseInspector => {
                self.current = self.inspector_return;
                SurfaceEffect::PageChanged(self.current)
            }
            SurfaceIntent::Technician(request) => {
                self.last_technician_intent = Some(request);
                SurfaceEffect::TechnicianIntentRecorded(request)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use blob_core::{
        BindingPlanId, CausalKind, CausalRecordId, CapabilityId, EffectEnvelope, EventId,
        ImplementationId, NodeId, RequirementGraphId, RequirementRoleId, ResolvedCapabilityRole,
        ResolutionTrace, SituationId, SituationWindow, TaskId,
    };

    use super::*;

    fn context(task_state: TaskState) -> (Situation, Task, BindingPlan, Vec<CausalRecord>) {
        let graph = RequirementGraphId::from("rg:test");
        let situation = Situation {
            id: SituationId::from("situation:test"),
            kind: "development.source-modified".into(),
            summary: "Source changed, so tests were reconsidered.".into(),
            evidence_event_ids: vec![EventId::from("event:test")],
            derived_facts: vec![("path".into(), "src/lib.rs".into())],
            subjects: vec![],
            window: SituationWindow {
                start_unix_ms: 1,
                end_unix_ms: 2,
            },
            confidence_ppm: Some(1_000_000),
            deterministic_rule_provenance: vec!["development.source-change-requires-test@v1".into()],
            semantic_provenance: vec![],
            expires_at_unix_ms: None,
        };
        let task = Task {
            id: TaskId::from("task:test"),
            title: "Run tests".into(),
            state: task_state,
            input_objects: vec![],
            requirement_graphs: vec![graph.clone()],
            output_objects: vec![],
            requested_effects: EffectEnvelope::default(),
        };
        let plan = BindingPlan {
            id: BindingPlanId::from("binding:test"),
            graph,
            resolved_capabilities: vec![ResolvedCapabilityRole {
                role: RequirementRoleId::from("role:test"),
                capability: CapabilityId::from("test.run"),
                implementation: ImplementationId::from("impl:test"),
                node: NodeId::from("node:local"),
            }],
            grants: vec![],
            data_routes: vec![],
            expected_effects: EffectEnvelope::default(),
            trace: ResolutionTrace {
                verifier_notes: vec!["verified exact test.run binding".into()],
                ..ResolutionTrace::default()
            },
        };
        let records = vec![CausalRecord {
            id: CausalRecordId::from("cause:test"),
            kind: CausalKind::ExecutionCompleted,
            occurred_at_unix_ms: 3,
            parents: vec![],
            actor: "blob-mvp".into(),
            summary: "Tests completed".into(),
            why: "verified capability execution completed".into(),
            event: None,
            situation: Some(situation.id.clone()),
            task: Some(task.id.clone()),
            requirement_graph: None,
            binding_plan: Some(plan.id.clone()),
            binding_lease: None,
            improvement_proposal: None,
            evidence: vec![],
            expected_effects: vec![],
            actual_effects: vec![],
            authorization: None,
            rollback_reference: None,
        }];
        (situation, task, plan, records)
    }

    #[test]
    fn inspector_returns_to_the_surface_that_opened_it() {
        let mut app = SurfaceApplication::default();
        app.apply(SurfaceIntent::Navigate(PrimarySurface::CurrentWorkspace));
        assert_eq!(
            app.apply(SurfaceIntent::OpenInspector),
            SurfaceEffect::PageChanged(PrimarySurface::Inspector)
        );
        assert_eq!(
            app.apply(SurfaceIntent::CloseInspector),
            SurfaceEffect::PageChanged(PrimarySurface::CurrentWorkspace)
        );
    }

    #[test]
    fn technician_prepare_records_intent_without_navigating_or_executing() {
        let mut app = SurfaceApplication::default();
        let before = app.current();
        assert_eq!(
            app.apply(SurfaceIntent::Technician(TechnicianIntent::PrepareNextStep)),
            SurfaceEffect::TechnicianIntentRecorded(TechnicianIntent::PrepareNextStep)
        );
        assert_eq!(app.current(), before);
        assert_eq!(
            app.last_technician_intent(),
            Some(TechnicianIntent::PrepareNextStep)
        );
    }

    #[test]
    fn navigation_is_explicit_and_renderer_independent() {
        let mut app = SurfaceApplication::default();
        for page in [
            PrimarySurface::History,
            PrimarySurface::Fabric,
            PrimarySurface::Now,
        ] {
            assert_eq!(
                app.apply(SurfaceIntent::Navigate(page)),
                SurfaceEffect::PageChanged(page)
            );
            assert_eq!(app.current(), page);
        }
    }

    #[test]
    fn explain_projection_uses_binding_verifier_and_causal_evidence() {
        let (situation, task, plan, records) = context(TaskState::Completed);
        let projection = TechnicianEvidenceContext {
            situation: &situation,
            task: &task,
            plan: &plan,
            causal_records: &records,
        }
        .project(TechnicianIntent::ExplainCurrent)
        .unwrap();

        assert!(projection.summary.contains("Completed"));
        assert!(projection.details.iter().any(|line| line.contains("test.run")));
        assert!(projection.details.iter().any(|line| line.contains("Verifier:")));
        assert!(projection.details.iter().any(|line| line.contains("Tests completed")));
        assert_eq!(projection.required_autonomy, TechnicianAutonomy::Observe);
    }

    #[test]
    fn completed_activity_prepare_preview_does_not_infer_a_system_change() {
        let (situation, task, plan, records) = context(TaskState::Completed);
        let projection = TechnicianEvidenceContext {
            situation: &situation,
            task: &task,
            plan: &plan,
            causal_records: &records,
        }
        .project(TechnicianIntent::PrepareNextStep)
        .unwrap();

        assert!(projection.summary.contains("will not invent a system change"));
        assert_eq!(projection.required_autonomy, TechnicianAutonomy::Prepare);
        assert!(
            projection
                .safety_notes
                .iter()
                .any(|line| line.contains("not permission"))
        );
    }

    #[test]
    fn projection_rejects_a_binding_from_another_requirement_graph() {
        let (situation, task, mut plan, records) = context(TaskState::Completed);
        plan.graph = RequirementGraphId::from("rg:other");
        assert_eq!(
            TechnicianEvidenceContext {
                situation: &situation,
                task: &task,
                plan: &plan,
                causal_records: &records,
            }
            .project(TechnicianIntent::ExplainCurrent),
            Err(TechnicianProjectionError::BindingDoesNotBelongToTaskRequirement)
        );
    }
}
