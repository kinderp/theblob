#![forbid(unsafe_code)]

use blob_core::{
    BindingPlan, CausalKind, CausalRecord, ImprovementProposal, ImprovementProposalId,
    PhysicalTestNodeProfile, PhysicalTestNodeReadiness, PhysicalTestNodeViolation, Situation,
    SystemCandidateAction, Task, TaskState, TechnicianAutonomy,
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
pub struct PhysicalNodePreflightReport {
    pub title: String,
    pub action: SystemCandidateAction,
    pub eligible: bool,
    pub summary: String,
    pub evidence: Vec<String>,
    pub blocking_reasons: Vec<String>,
    pub probe_warnings: Vec<String>,
    pub next_steps: Vec<String>,
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
    pub fn explain_system_change(
        record: &CausalRecord,
    ) -> Result<SystemChangeReport, TechnicianError> {
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

    /// Explain the deterministic physical-node readiness gate. The Technician
    /// does not create or override readiness evidence and passing this report is
    /// explicitly not equivalent to granting administrator authority.
    pub fn explain_physical_node_preflight(
        profile: &PhysicalTestNodeProfile,
        readiness: &PhysicalTestNodeReadiness,
        action: SystemCandidateAction,
        probe_warnings: &[String],
    ) -> PhysicalNodePreflightReport {
        let blocking_reasons = match profile.validate_readiness(&action, readiness) {
            Ok(()) => Vec::new(),
            Err(violations) => violations
                .iter()
                .map(physical_violation_explanation)
                .collect(),
        };
        let eligible = blocking_reasons.is_empty();

        let summary = if eligible {
            eligible_summary(&action)
        } else {
            format!(
                "The node is not ready for {:?}: {} deterministic prerequisite(s) are unsatisfied.",
                action,
                blocking_reasons.len()
            )
        };

        let next_steps = if eligible {
            eligible_next_steps(&action)
        } else {
            vec![
                "Resolve the blocking prerequisites and collect a fresh readiness observation."
                    .into(),
                "Do not bypass a failed readiness gate with an AI judgement or ad-hoc privileged shell command."
                    .into(),
            ]
        };

        let mut safety_notes = vec![
            "This preflight is read-only: it does not execute the requested system action."
                .into(),
            "Readiness eligibility is separate from authorization; administrator authority is still required for privileged actions."
                .into(),
            "Persistent boot/switch activation is outside the current Linux Pilot authority model."
                .into(),
        ];
        if !probe_warnings.is_empty() {
            safety_notes.push(
                "Probe warnings are preserved as uncertainty and are not silently treated as satisfied safety facts."
                    .into(),
            );
        }

        PhysicalNodePreflightReport {
            title: format!("Physical node preflight for {:?}", action),
            action,
            eligible,
            summary,
            evidence: readiness.evidence_lines(),
            blocking_reasons,
            probe_warnings: probe_warnings.to_vec(),
            next_steps,
            safety_notes,
        }
    }
}

fn physical_violation_explanation(violation: &PhysicalTestNodeViolation) -> String {
    match violation {
        PhysicalTestNodeViolation::NodeMismatch => {
            "The readiness evidence belongs to a different enrolled node.".into()
        }
        PhysicalTestNodeViolation::ArchitectureMismatch => {
            "The observed CPU architecture does not match the physical test-node profile.".into()
        }
        PhysicalTestNodeViolation::SubstrateMismatch => {
            "The observed operating-system substrate does not match the physical test-node profile."
                .into()
        }
        PhysicalTestNodeViolation::NotEnrolled => {
            "The machine has not been enrolled as a physical test node.".into()
        }
        PhysicalTestNodeViolation::NotTrusted => {
            "The machine enrollment is not trusted for system experimentation.".into()
        }
        PhysicalTestNodeViolation::StorageHealthNotConfirmed => {
            "Storage health has not been confirmed.".into()
        }
        PhysicalTestNodeViolation::InsufficientFreeSpace {
            required_bytes,
            observed_bytes,
        } => format!(
            "Insufficient free storage: at least {required_bytes} bytes are required, but {observed_bytes} bytes were observed."
        ),
        PhysicalTestNodeViolation::PreviewActivationDisabled => {
            "Preview activation is disabled by the physical test-node profile.".into()
        }
        PhysicalTestNodeViolation::TestActivationDisabled => {
            "Temporary live test activation is disabled by the physical test-node profile.".into()
        }
        PhysicalTestNodeViolation::ExternalPowerRequired => {
            "External power has not been confirmed for temporary live activation.".into()
        }
        PhysicalTestNodeViolation::LocalConsoleRecoveryNotConfirmed => {
            "Local-console recovery has not been physically confirmed.".into()
        }
        PhysicalTestNodeViolation::CurrentBootGenerationUnknown => {
            "The current boot-default generation is unknown.".into()
        }
        PhysicalTestNodeViolation::RollbackReferenceMissing => {
            "No explicit rollback reference is recorded for the current boot-default state.".into()
        }
    }
}

fn eligible_summary(action: &SystemCandidateAction) -> String {
    match action {
        SystemCandidateAction::Materialize => {
            "The node is ready to materialize the candidate without changing the live system.".into()
        }
        SystemCandidateAction::BuildIsolatedVm => {
            "The node is ready to build an isolated VM candidate without changing the live system.".into()
        }
        SystemCandidateAction::PreviewActivation => {
            "The node satisfies the readiness prerequisites for an administrator-authorized activation preview."
                .into()
        }
        SystemCandidateAction::TestActivation => {
            "The node satisfies the readiness prerequisites for an administrator-authorized temporary live test activation."
                .into()
        }
    }
}

fn eligible_next_steps(action: &SystemCandidateAction) -> Vec<String> {
    match action {
        SystemCandidateAction::Materialize => vec![
            "Use the bounded non-privileged materialization executor and record the result as causal evidence."
                .into(),
        ],
        SystemCandidateAction::BuildIsolatedVm => vec![
            "Build and boot the isolated candidate before considering any physical activation."
                .into(),
        ],
        SystemCandidateAction::PreviewActivation => vec![
            "Request explicit host-administrator authorization before executing the preview."
                .into(),
            "Capture any dry-activation hook effects as evidence; do not infer that preview equals test activation."
                .into(),
        ],
        SystemCandidateAction::TestActivation => vec![
            "Request explicit host-administrator authorization before temporary live activation."
                .into(),
            "Keep the recorded rollback reference and local console available throughout the test."
                .into(),
            "Do not make the candidate the persistent boot default under the current v0.1 policy."
                .into(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use blob_core::{
        CausalKind, CausalRecordId, NodeId, PhysicalNodeSubstrate, SystemArchitecture,
    };

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

    fn physical_profile() -> PhysicalTestNodeProfile {
        PhysicalTestNodeProfile::nixos_pilot("node:lab", SystemArchitecture::X86_64)
    }

    fn physical_readiness() -> PhysicalTestNodeReadiness {
        PhysicalTestNodeReadiness {
            node: NodeId::from("node:lab"),
            observed_architecture: SystemArchitecture::X86_64,
            observed_substrate: PhysicalNodeSubstrate::NixOs,
            enrolled: true,
            trusted: true,
            on_external_power: true,
            free_space_bytes: 16 * 1024 * 1024 * 1024,
            storage_health_ok: true,
            current_boot_generation: Some("nixos-generation:42".into()),
            rollback_reference: Some("nixos-generation:42".into()),
            local_console_recovery_confirmed: true,
            observed_at_unix_ms: 1,
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
        assert!(report
            .safety_notes
            .iter()
            .any(|note| note.contains("did not change the live system")));
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

    #[test]
    fn preflight_explains_why_live_activation_is_blocked() {
        let mut readiness = physical_readiness();
        readiness.on_external_power = false;
        readiness.rollback_reference = None;

        let report = ReadOnlySystemTechnician::explain_physical_node_preflight(
            &physical_profile(),
            &readiness,
            SystemCandidateAction::TestActivation,
            &["power-state-unavailable".into()],
        );

        assert!(!report.eligible);
        assert!(report
            .blocking_reasons
            .iter()
            .any(|reason| reason.contains("External power")));
        assert!(report
            .blocking_reasons
            .iter()
            .any(|reason| reason.contains("rollback reference")));
        assert_eq!(report.probe_warnings, vec!["power-state-unavailable"]);
    }

    #[test]
    fn materialization_preflight_does_not_demand_live_recovery_facts() {
        let mut readiness = physical_readiness();
        readiness.on_external_power = false;
        readiness.current_boot_generation = None;
        readiness.rollback_reference = None;
        readiness.local_console_recovery_confirmed = false;

        let report = ReadOnlySystemTechnician::explain_physical_node_preflight(
            &physical_profile(),
            &readiness,
            SystemCandidateAction::Materialize,
            &[],
        );

        assert!(report.eligible);
        assert!(report.summary.contains("without changing the live system"));
    }

    #[test]
    fn passing_live_preflight_is_not_described_as_authorization() {
        let report = ReadOnlySystemTechnician::explain_physical_node_preflight(
            &physical_profile(),
            &physical_readiness(),
            SystemCandidateAction::TestActivation,
            &[],
        );

        assert!(report.eligible);
        assert!(report
            .summary
            .contains("administrator-authorized temporary live test activation"));
        assert!(report.safety_notes.iter().any(|note| {
            note.contains("separate from authorization")
        }));
    }
}
