#![forbid(unsafe_code)]

use blob_core::{
    BindingPlan, CausalKind, CausalRecord, ImprovementProposal, ImprovementProposalId, Situation,
    Task, TaskState, TechnicianAutonomy,
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
pub struct SystemChangeReport {
    pub title: String,
    pub summary: String,
    pub why: String,
    pub evidence: Vec<String>,
    pub expected_effects: Vec<String>,
    pub actual_effects: Vec<String>,
    pub authorization: Option<String>,
    pub rollback_reference: Option<String>,
    pub safety_notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TechnicianError {
    UnsupportedSituation(String),
    UnsupportedCausalKind(CausalKind),
    TaskIsNotFailed,
    BindingDoesNotBelongToTaskRequirement,
}

/// Read-only System Technician.
///
/// This type intentionally has no executor, package-manager, SystemSpec or
/// privileged mutation dependency. It can only transform already-structured
/// evidence into human-facing reports/proposals. System-specific backends must
/// first publish backend-neutral causal evidence before the Technician explains it.
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

    /// Explain an already-recorded system change without acquiring any execution
    /// dependency or authority. This is deliberately backend-neutral: NixOS,
    /// macOS, Windows or future substrates can all publish `SystemChange` causal
    /// records and reuse this explanation path.
    pub fn explain_system_change(record: &CausalRecord) -> Result<SystemChangeReport, TechnicianError> {
        if record.kind != CausalKind::SystemChange {
            return Err(TechnicianError::UnsupportedCausalKind(record.kind.clone()));
        }

        let materialization_only = record
            .evidence
            .iter()
            .any(|line| line == "effect-class:MaterializationOnly");

        let mut safety_notes = vec![
            "This report is read-only and cannot activate, roll back or otherwise mutate the system."
                .into(),
        ];
        if materialization_only {
            safety_notes.push(
                "The recorded operation only materialized a candidate; it did not change the live system."
                    .into(),
            );
        }
        if record.rollback_reference.is_none() && !materialization_only {
            safety_notes.push(
                "No rollback reference is attached to this system-change record; activation should not be inferred safe from this report."
                    .into(),
            );
        }

        Ok(SystemChangeReport {
            title: "System candidate operation".into(),
            summary: record.summary.clone(),
            why: record.why.clone(),
            evidence: record.evidence.clone(),
            expected_effects: record.expected_effects.clone(),
            actual_effects: record.actual_effects.clone(),
            authorization: record.authorization.clone(),
            rollback_reference: record.rollback_reference.clone(),
            safety_notes,
        })
    }
}

#[cfg(test)]
mod tests {
    use blob_core::{CausalRecordId, CausalKind};

    use super::*;

    fn system_change_record() -> CausalRecord {
        CausalRecord {
            id: CausalRecordId::from("causal:system:test"),
            kind: CausalKind::SystemChange,
            occurred_at_unix_ms: 1,
            parents: Vec::new(),
            actor: "blob-system-executor".into(),
            summary: "Materialize for candidate:test succeeded".into(),
            why: "build the candidate before any activation".into(),
            event: None,
            situation: None,
            task: None,
            requirement_graph: None,
            binding_plan: None,
            binding_lease: None,
            improvement_proposal: None,
            evidence: vec![
                "effect-class:MaterializationOnly".into(),
                "nix-store-path:/nix/store/abc-system".into(),
            ],
            expected_effects: vec!["materialize immutable candidate".into()],
            actual_effects: vec!["materialized:/nix/store/abc-system".into()],
            authorization: Some("user/non-privileged materialization".into()),
            rollback_reference: None,
        }
    }

    #[test]
    fn explains_materialization_without_backend_dependency() {
        let report = ReadOnlySystemTechnician::explain_system_change(&system_change_record())
            .expect("system change must be explainable");

        assert!(report.summary.contains("succeeded"));
        assert!(report
            .evidence
            .iter()
            .any(|line| line.contains("/nix/store/abc-system")));
        assert!(report.safety_notes.iter().any(|note| {
            note.contains("did not change the live system")
        }));
        assert!(report.rollback_reference.is_none());
    }

    #[test]
    fn rejects_non_system_change_records() {
        let mut record = system_change_record();
        record.kind = CausalKind::TaskTransition;

        assert_eq!(
            ReadOnlySystemTechnician::explain_system_change(&record),
            Err(TechnicianError::UnsupportedCausalKind(
                CausalKind::TaskTransition
            ))
        );
    }
}
