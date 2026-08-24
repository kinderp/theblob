use blob_alfred::{Alfred, SourceChangeRequiresTestRule};
use blob_core::{
    Event, EventId, EventSource, EventTrustClass, SubjectRef, WorkspaceId,
};

fn source_modified_event(workspace: &WorkspaceId) -> Event {
    Event {
        id: EventId::from("event:source-1"),
        schema_version: 1,
        source: EventSource::FileSystem,
        source_node: None,
        kind: "source.modified".into(),
        subjects: vec![SubjectRef::Workspace(workspace.clone())],
        observed_at_unix_ms: 1_000,
        received_at_unix_ms: 1_001,
        source_sequence: Some(42),
        correlation_keys: vec!["repo:demo".into()],
        attributes: vec![("path".into(), "src/lib.rs".into())],
        trust: EventTrustClass::TrustedLocalService,
        provenance: vec!["fixture:file-watcher".into()],
        causal_parent_ids: vec![],
    }
}

#[test]
fn relevant_source_change_produces_deterministic_test_situation() {
    let workspace = WorkspaceId::from("ws:development");
    let rule = SourceChangeRequiresTestRule::v1(workspace.clone());
    let mut alfred = Alfred::new(vec![Box::new(rule)]);

    let outcome = alfred.ingest(source_modified_event(&workspace));

    assert!(!outcome.duplicate);
    assert_eq!(outcome.situations.len(), 1);
    assert_eq!(
        outcome.situations[0].kind,
        "development.source-change-requires-test"
    );
    assert_eq!(
        outcome.situations[0].deterministic_rule_provenance,
        vec!["development.source-change-requires-test@v1"]
    );
    assert!(outcome.situations[0].semantic_provenance.is_empty());
}

#[test]
fn replayed_event_is_deduplicated() {
    let workspace = WorkspaceId::from("ws:development");
    let rule = SourceChangeRequiresTestRule::v1(workspace.clone());
    let mut alfred = Alfred::new(vec![Box::new(rule)]);
    let event = source_modified_event(&workspace);

    let first = alfred.ingest(event.clone());
    let second = alfred.ingest(event);

    assert!(!first.duplicate);
    assert!(second.duplicate);
    assert!(second.situations.is_empty());
    assert_eq!(alfred.seen_event_count(), 1);
}

#[test]
fn unrelated_workspace_does_not_trigger_rule() {
    let active = WorkspaceId::from("ws:active");
    let other = WorkspaceId::from("ws:other");
    let rule = SourceChangeRequiresTestRule::v1(active);
    let mut alfred = Alfred::new(vec![Box::new(rule)]);

    let outcome = alfred.ingest(source_modified_event(&other));

    assert!(!outcome.duplicate);
    assert!(outcome.situations.is_empty());
}
