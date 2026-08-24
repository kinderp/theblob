use std::collections::HashMap;
use std::env;

use blob_core::{
    CapabilityId, CapabilityImplementation, ControlDepth, Event, EventId, EventSource,
    EventTrustClass, ImplementationId, NodeFacts, NodeId, RuntimeKind, SubjectRef, WorkspaceId,
};
use blob_executor::LocalProcessCapsule;
use blob_mvp::run_source_change_test;
use blob_resolver::Registry;
use blob_surface_slint::{DevelopmentSurfaceSnapshot, show};

fn main() -> Result<(), slint::PlatformError> {
    let workspace = WorkspaceId::from("ws:development-demo");
    let node_id = NodeId::from("node:local-demo");
    let implementation_id = ImplementationId::from("impl:test-process-demo");
    let platform = format!("{}-{}", env::consts::ARCH, env::consts::OS);

    let event = Event {
        id: EventId::from("event:surface-demo-source-change"),
        schema_version: 1,
        source: EventSource::FileSystem,
        source_node: Some(node_id.clone()),
        kind: "source.modified".into(),
        subjects: vec![SubjectRef::Workspace(workspace.clone())],
        observed_at_unix_ms: 1_000,
        received_at_unix_ms: 1_001,
        source_sequence: Some(1),
        correlation_keys: vec!["repo:surface-demo".into()],
        attributes: vec![("path".into(), "src/lib.rs".into())],
        trust: EventTrustClass::TrustedLocalService,
        provenance: vec!["demo:file-watcher".into()],
        causal_parent_ids: vec![],
    };

    let registry = Registry {
        implementations: vec![CapabilityImplementation {
            id: implementation_id.clone(),
            implements: CapabilityId::from("test.run"),
            runtime: RuntimeKind::Native,
            trusted: true,
            supported_platforms: vec![platform.clone()],
            required_memory_bytes: 16 * 1024 * 1024,
            required_accelerators: vec![],
            network_required: false,
            quality_ppm: 1_000_000,
            expected_latency_us: 10_000,
            expected_energy_uj: 1_000,
            cost_microeur: 0,
        }],
        nodes: vec![NodeFacts {
            id: node_id,
            platform,
            trusted: true,
            online: true,
            memory_bytes: 8 * 1024 * 1024 * 1024,
            accelerator_tags: vec![],
            runtime_tags: vec!["native".into()],
            data_residency: "personal-fabric".into(),
            control_depth: if env::consts::OS == "macos" {
                ControlDepth::Hosted
            } else {
                ControlDepth::SystemMutable
            },
        }],
    };

    let process_capsules = HashMap::from([(
        implementation_id.clone(),
        LocalProcessCapsule {
            implementation: implementation_id,
            program: "sh".into(),
            args: vec!["-c".into(), "printf 'tests-ok'".into()],
        },
    )]);

    let outcome = run_source_change_test(
        event,
        workspace,
        &registry,
        &process_capsules,
        env::temp_dir(),
    )
    .expect("the validated MVP demo flow should complete before rendering");

    let snapshot = DevelopmentSurfaceSnapshot::from_vertical_slice(&outcome);
    show(&snapshot)
}
