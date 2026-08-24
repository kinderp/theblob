use blob_core::*;
use blob_resolver::{BindingVerifier, Registry, ResolveError, Resolver};

fn graph() -> RequirementGraph {
    RequirementGraph {
        id: RequirementGraphId::from("rg:test-run"),
        roles: vec![RequirementRole {
            id: RequirementRoleId::from("role:test-runner"),
            kind: RequirementRoleKind::Capability(CapabilityId::from("test.run")),
        }],
        relations: vec![],
        constraints: vec![],
        requested_effects: EffectEnvelope {
            effects: vec![EffectSpec {
                kind: EffectKind::Execute,
                target_type: None,
                reversible: true,
            }],
        },
    }
}

fn implementation(id: &str, quality_ppm: u32, cost_microeur: u64) -> CapabilityImplementation {
    CapabilityImplementation {
        id: ImplementationId::from(id),
        implements: CapabilityId::from("test.run"),
        runtime: RuntimeKind::Native,
        trusted: true,
        supported_platforms: vec!["x86_64-linux".into()],
        required_memory_bytes: 64 * 1024 * 1024,
        required_accelerators: vec![],
        network_required: false,
        quality_ppm,
        expected_latency_us: 50_000,
        expected_energy_uj: 10_000,
        cost_microeur,
    }
}

fn node(id: &str) -> NodeFacts {
    NodeFacts {
        id: NodeId::from(id),
        platform: "x86_64-linux".into(),
        trusted: true,
        online: true,
        memory_bytes: 8 * 1024 * 1024 * 1024,
        accelerator_tags: vec![],
        runtime_tags: vec!["native".into()],
        data_residency: "personal-fabric".into(),
        control_depth: ControlDepth::SystemMutable,
    }
}

#[test]
fn resolver_prefers_higher_quality_before_lower_cost() {
    let registry = Registry {
        implementations: vec![
            implementation("impl:cheap", 900_000, 0),
            implementation("impl:better", 950_000, 100),
        ],
        nodes: vec![node("node:local")],
    };

    let (plan, lease) = Resolver::new(&registry)
        .resolve_single_capability(&graph())
        .expect("binding should resolve");

    assert_eq!(plan.resolved_capabilities.len(), 1);
    assert_eq!(
        plan.resolved_capabilities[0].implementation,
        ImplementationId::from("impl:better")
    );
    assert_eq!(lease.plan, plan.id);
    assert_eq!(plan.trace.solver_backend.as_deref(), Some("rust-mvp-deterministic@v1"));
    assert!(plan
        .trace
        .verifier_notes
        .iter()
        .any(|note| note.contains("independently verified")));
}

#[test]
fn offline_or_untrusted_candidates_are_rejected() {
    let mut offline = node("node:offline");
    offline.online = false;

    let registry = Registry {
        implementations: vec![implementation("impl:test", 1_000_000, 0)],
        nodes: vec![offline],
    };

    let result = Resolver::new(&registry).resolve_single_capability(&graph());
    assert_eq!(result, Err(ResolveError::NoCandidate));
}

#[test]
fn independent_verifier_rejects_tampered_plan() {
    let registry = Registry {
        implementations: vec![implementation("impl:test", 1_000_000, 0)],
        nodes: vec![node("node:local")],
    };
    let requirement_graph = graph();
    let (mut plan, _) = Resolver::new(&registry)
        .resolve_single_capability(&requirement_graph)
        .expect("binding should resolve");

    plan.resolved_capabilities[0].node = NodeId::from("node:does-not-exist");

    let report = BindingVerifier::verify(&registry, &requirement_graph, &plan);
    assert!(!report.is_valid());
    assert!(report.errors.iter().any(|error| error.contains("not found")));
}

#[test]
fn constraint_ir_is_explicitly_rejected_until_supported() {
    let mut requirement_graph = graph();
    requirement_graph.constraints.push(Constraint {
        class: ConstraintClass::Hard,
        expression: ConstraintExpr::Ge("quality_ppm".into(), 900_000),
        explanation: "minimum quality".into(),
    });

    let registry = Registry {
        implementations: vec![implementation("impl:test", 1_000_000, 0)],
        nodes: vec![node("node:local")],
    };

    let result = Resolver::new(&registry).resolve_single_capability(&requirement_graph);
    assert!(matches!(result, Err(ResolveError::UnsupportedGraph(_))));
}
