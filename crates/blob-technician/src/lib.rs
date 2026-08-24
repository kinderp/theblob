#![forbid(unsafe_code)]

use blob_core::{
    BindingPlan, ImprovementProposal, ImprovementProposalId, Situation, Task, TaskState,
    TechnicianAutonomy,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticReport {
    pub title: String,
    pub summary: String,
    pub local_evidence: Vec<String>,
    pub binding_explanation: Vec<String>,
    pub uncertainty: Vec<String>,
    pub suggested_next_steps: Vec<String>,
    pub proposal: ImprovementProposal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TechnicianError {
    UnsupportedSituation(String),
    TaskIsNotFailed,
    BindingDoesNotBelongToTaskRequirement,
}

/// Read-only Phase 1 Technician.
///
/// This type intentionally has no executor, package-manager, SystemSpec or
/// privileged mutation dependency. It can only transform already-structured
/// evidence into a diagnostic report/proposal.
pub struct ReadOnlySystemTechnician;

impl ReadOnlySystemTechnician {
    pub fn diagnose_failed_test(
        situation: &Situation,
        task: &Task,
        plan: &BindingPlan,
    ) -> Result<DiagnosticReport, TechnicianError> {
        if situation.kind != "development.test-failed" {
            return Err(TechnicianError::UnsupportedSituation(
                situation.kind.clone(),
            ));
        }

        if task.state != TaskState::Failed {
            return Err(TechnicianError::TaskIsNotFailed);
        }

        if !task.requirement_graphs.iter().any(|graph| graph == &plan.graph) {
            return Err(TechnicianError::BindingDoesNotBelongToTaskRequirement);
        }

        let mut local_evidence = situation
            .evidence_event_ids
            .iter()
            .map(|event| format!("evidence event: {event}"))
            .collect::<Vec<_>>();

        for (key, value) in &situation.derived_facts {
            local_evidence.push(format!("{key}={value}"));
        }

        let binding_explanation = if plan.resolved_capabilities.is_empty() {
            vec!["No resolved capability is present in the BindingPlan.".into()]
        } else {
            plan.resolved_capabilities
                .iter()
                .map(|resolved| {
                    format!(
                        "{} used implementation {} on node {}",
                        resolved.capability, resolved.implementation, resolved.node
                    )
                })
                .chain(plan.trace.verifier_notes.iter().cloned())
                .collect()
        };

        let proposal = ImprovementProposal {
            id: ImprovementProposalId::new(format!("proposal:diagnose:{}", task.id)),
            title: format!("Diagnose failed tests for {}", task.title),
            triggering_situations: vec![situation.id.clone()],
            local_evidence: local_evidence.clone(),
            external_evidence: Vec::new(),
            applicability_reasoning: "The deterministic Situation identifies a failed test.run for this failed Task, and the supplied BindingPlan belongs to one of the Task RequirementGraphs."
                .into(),
            proposed_changes: vec![
                "Inspect the structured failure output and identify the smallest failing test scope."
                    .into(),
                "After a source-level correction, rerun test.run under a newly verified BindingLease."
                    .into(),
            ],
            expected_benefits: vec![
                "Reduce diagnostic ambiguity before changing toolchain, runtime or system state."
                    .into(),
            ],
            risks: vec![
                "The current evidence does not establish that the failure is caused by the selected implementation, node, toolchain or operating system."
                    .into(),
            ],
            test_plan: vec![
                "Reproduce the failing test deterministically before proposing system-level changes."
                    .into(),
            ],
            rollback_reference: None,
            required_autonomy: TechnicianAutonomy::Suggest,
            expires_at_unix_ms: None,
        };

        Ok(DiagnosticReport {
            title: proposal.title.clone(),
            summary: "Tests failed. The evidence is sufficient to recommend focused diagnosis, but not sufficient to justify a system mutation."
                .into(),
            local_evidence,
            binding_explanation,
            uncertainty: proposal.risks.clone(),
            suggested_next_steps: proposal.proposed_changes.clone(),
            proposal,
        })
    }
}
