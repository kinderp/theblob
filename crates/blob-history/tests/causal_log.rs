use blob_core::{CausalKind, CausalRecord, CausalRecordId, EventId, SituationId};
use blob_history::{HistoryError, InMemoryCausalLog};

fn record(id: &str, parents: Vec<CausalRecordId>) -> CausalRecord {
    CausalRecord {
        id: CausalRecordId::from(id),
        kind: CausalKind::EventObserved,
        occurred_at_unix_ms: 1_000,
        parents,
        actor: "alfred".into(),
        summary: "fixture".into(),
        why: "test causal chain".into(),
        event: Some(EventId::from("event:fixture")),
        situation: None,
        task: None,
        requirement_graph: None,
        binding_plan: None,
        binding_lease: None,
        improvement_proposal: None,
        evidence: vec![],
        expected_effects: vec![],
        actual_effects: vec![],
        authorization: None,
        rollback_reference: None,
    }
}

#[test]
fn causal_log_accepts_parent_before_child() {
    let mut log = InMemoryCausalLog::default();
    let root_id = CausalRecordId::from("cause:root");
    let child_id = CausalRecordId::from("cause:child");

    log.append(record(root_id.as_str(), vec![])).unwrap();

    let mut child = record(child_id.as_str(), vec![root_id.clone()]);
    child.kind = CausalKind::SituationDerived;
    child.situation = Some(SituationId::from("situation:test"));
    log.append(child).unwrap();

    assert_eq!(log.len(), 2);
    assert_eq!(log.get(&child_id).unwrap().parents, vec![root_id]);
}

#[test]
fn causal_log_rejects_unknown_parent() {
    let mut log = InMemoryCausalLog::default();
    let missing = CausalRecordId::from("cause:missing");
    let error = log
        .append(record("cause:child", vec![missing.clone()]))
        .expect_err("unknown parent must fail");

    assert_eq!(error, HistoryError::UnknownParent(missing));
}

#[test]
fn causal_log_rejects_duplicate_ids() {
    let mut log = InMemoryCausalLog::default();
    log.append(record("cause:dup", vec![])).unwrap();

    let error = log
        .append(record("cause:dup", vec![]))
        .expect_err("duplicate ID must fail");

    assert_eq!(error, HistoryError::DuplicateId(CausalRecordId::from("cause:dup")));
}
