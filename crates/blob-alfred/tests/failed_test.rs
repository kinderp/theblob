use blob_alfred::{Alfred, TestFailedRule};
use blob_core::{
    Event, EventId, EventSource, EventTrustClass, SubjectRef, TaskId,
};

fn failed_test_event() -> Event {
    Event {
        id: EventId::from("event:test-failed"),
        schema_version: 1,
        source: EventSource::Capability,
        source_node: None,
        kind: "capability.failed".into(),
        subjects: vec![SubjectRef::Task(TaskId::from("task:test"))],
        observed_at_unix_ms: 2_000,
        received_at_unix_ms: 2_001,
        source_sequence: Some(9),
        correlation_keys: vec!["task:test".into()],
        attributes: vec![
            ("capability".into(), "test.run".into()),
            ("exit_code".into(), "7".into()),
            ("implementation".into(), "impl:test".into()),
            ("node".into(), "node:local".into()),
        ],
        trust: EventTrustClass::TrustedLocalService,
        provenance: vec!["blob-executor".into()],
        causal_parent_ids: vec![],
    }
}

#[test]
fn capability_failure_for_test_run_becomes_failed_test_situation() {
    let mut alfred = Alfred::new(vec![Box::new(TestFailedRule::v1())]);

    let outcome = alfred.ingest(failed_test_event());

    assert_eq!(outcome.situations.len(), 1);
    let situation = &outcome.situations[0];
    assert_eq!(situation.kind, "development.test-failed");
    assert!(situation
        .derived_facts
        .iter()
        .any(|(key, value)| key == "exit_code" && value == "7"));
    assert_eq!(
        situation.deterministic_rule_provenance,
        vec!["development.test-failed@v1"]
    );
    assert!(situation.semantic_provenance.is_empty());
}

#[test]
fn non_test_capability_failure_does_not_match() {
    let mut event = failed_test_event();
    event.attributes[0] = ("capability".into(), "build.run".into());
    let mut alfred = Alfred::new(vec![Box::new(TestFailedRule::v1())]);

    let outcome = alfred.ingest(event);

    assert!(outcome.situations.is_empty());
}
