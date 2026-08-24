use std::collections::HashMap;
use std::env;

use blob_core::{
    CapabilityId, CapabilityImplementation, ControlDepth, Event, EventId, EventSource,
    EventTrustClass, ImplementationId, NodeFacts, NodeId, RuntimeKind, SubjectRef, TaskState,
    WorkspaceId,
};
use blob_executor::{ExecutionStatus, LocalProcessCapsule};
use blob_mvp::run_source_change_test;
use blob_resolver::Registry;

#[test]
fn source_change_reaches_verified_ephemeral_execution_and_causal_record() {
    let workspace = WorkspaceId::from("ws:development");
    let event = Event {
        id: EventId::from("event:source-change"),
        schema_version: 1,
        source: EventSource::FileSystem,
        source_node: Some(NodeId::from("node:local")),
        kind: "source.modified".into(),
        subjects: vec![SubjectRef::Workspace(workspace.clone())],
        observed_at_unix_ms: 1_000,
        received_at_unix_ms: 1_001,
        source_sequence: Some(1),
        correlation_keys: vec!["repo:fixture".into()],
        attributes: vec![("path".into(), "src/lib.rs".into())],
        trust: EventTrustClass::TrustedLocalService,
        provenance: vec!["fixture:file-watcher".into()],
        causal_parent_ids: vec![],
    };

    let implementation_id = ImplementationId::from("impl:test-process");
    let registry = Registry {
        implementations: vec![CapabilityImplementation {
            id: implementation_id.clone(),
            implements: CapabilityId::from("test.run"),
            runtime: RuntimeKind::Native,
            trusted: true,
            supported_platforms: vec!["x86_64-linux".into()],
            required_memory_bytes: 16 * 1024 * 1024,
            required_accelerators: vec![],
            network_required: false,
            quality_ppm: 1_000_000,
            expected_latency_us: 10_000,
            expected_energy_uj: 1_000,
            cost_microeur: 0,
        }],
        nodes: vec![NodeFacts {
            id: NodeId::from("node:local"),
            platform: "x86_64-linux".into(),
            trusted: true,
            online: true,
            memory_bytes: 8 * 1024 * 1024 * 1024,
            accelerator_tags: vec![],
            runtime_tags: vec!["native".into()],
            data_residency: "personal-fabric".into(),
            control_depth: ControlDepth::SystemMutable,
        }],
    };

    let process_capsules = HashMap::from([(
        implementation_id.clone(),
        LocalProcessCapsule {
            implementation: implementation_id,
            program: "sh".into(),
            args: vec!["-c".into(), "printf tests-ok".into()],
        },
    )]);

    let outcome = run_source_change_test(
        event,
        workspace,
        &registry,
        &process_capsules,
        env::temp_dir(),
    )
    .expect("vertical slice should succeed");

    assert_eq!(
        outcome.situation.kind,
        "development.source-change-requires-test"
    );
    assert_eq!(outcome.task.state, TaskState::Completed);
    assert_eq!(outcome.execution.status, ExecutionStatus::Succeeded);
    assert_eq!(outcome.execution.stdout, "tests-ok");
    assert_eq!(outcome.plan.resolved_capabilities.len(), 1);
    assert_eq!(outcome.lease.plan, outcome.plan.id);
    assert_eq!(outcome.causal_records.len(), 5);
    assert_eq!(
        outcome.causal_records.last().unwrap().summary,
        "test.run finished with Succeeded"
    );
}
