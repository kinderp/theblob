use blob_core::*;
use blob_technician::{ReadOnlySystemTechnician, TechnicianError};

fn failed_task() -> Task {
    Task {
        id: TaskId::from("task:failed-test"),
        title: "Run project tests".into(),
        state: TaskState::Failed,
        input_objects: vec![],
        requirement_graphs: vec![RequirementGraphId::from("rg:failed-test")],
        output_objects: vec![],
        requested_effects: EffectEnvelope::default(),
    }
}

fn failed_situation() -> Situation {
    Situation {
        id: SituationId::from("situation:test-failed"),
        kind: "development.test-failed".into(),
        summary: "test.run failed".into(),
        evidence_event_ids: vec![EventId::from("event:test-failed")],
        derived_facts: vec![
            ("test_failed".into(), "true".into()),
            ("exit_code".into(), "7".into()),
        ],
        subjects: vec![SubjectRef::Task(TaskId::from("task:failed-test"))],
        window: SituationWindow {
            start_unix_ms: 1_000,
            end_unix_ms: 1_000,
        },
        confidence_ppm: None,
        deterministic_rule_provenance: vec!["development.test-failed@v1".into()],
        semantic_provenance: vec![],
        expires_at_unix_ms: None,
    }
}

fn binding_plan() -> BindingPlan {
    BindingPlan {
        id: BindingPlanId::from("plan:failed-test"),
        graph: RequirementGraphId::from("rg:failed-test"),
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
            verifier_notes: vec!["binding independently verified".into()],
            ..ResolutionTrace::default()
        },
    }
}

#[test]
fn failed_test_yields_evidence_backed_suggest_only_report() {
    let report = ReadOnlySystemTechnician::diagnose_failed_test(
        &failed_situation(),
        &failed_task(),
        &binding_plan(),
    )
    .expect("failed test should be diagnosable");

    assert_eq!(report.proposal.required_autonomy, TechnicianAutonomy::Suggest);
    assert!(report.proposal.external_evidence.is_empty());
    assert!(report
        .local_evidence
        .iter()
        .any(|evidence| evidence.contains("exit_code=7")));
    assert!(report
        .binding_explanation
        .iter()
        .any(|line| line.contains("impl:test") && line.contains("node:local")));
    assert!(report
        .uncertainty
        .iter()
        .any(|risk| risk.contains("does not establish")));
}

#[test]
fn technician_refuses_to_diagnose_wrong_situation_kind() {
    let mut situation = failed_situation();
    situation.kind = "development.source-change-requires-test".into();

    let result = ReadOnlySystemTechnician::diagnose_failed_test(
        &situation,
        &failed_task(),
        &binding_plan(),
    );

    assert!(matches!(
        result,
        Err(TechnicianError::UnsupportedSituation(_))
    ));
}

#[test]
fn technician_refuses_non_failed_task() {
    let mut task = failed_task();
    task.state = TaskState::Completed;

    let result = ReadOnlySystemTechnician::diagnose_failed_test(
        &failed_situation(),
        &task,
        &binding_plan(),
    );

    assert_eq!(result, Err(TechnicianError::TaskIsNotFailed));
}
