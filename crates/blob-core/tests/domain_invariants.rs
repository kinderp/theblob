use blob_core::*;

fn grammar() -> ExperienceGrammar {
    ExperienceGrammar {
        semantic_roles: vec!["PrimaryEditor".into(), "TechnicianPanel".into()],
        navigation_model: "keyboard-first".into(),
        command_model: "palette".into(),
        stable_shortcuts: vec!["workspace.command".into()],
    }
}

#[test]
fn workspace_and_task_depend_on_capability_semantics_not_implementation_identity() {
    let capability = CapabilityId::from("test.run");
    let graph_id = RequirementGraphId::from("rg:test-1");

    let workspace = Workspace {
        id: WorkspaceId::from("ws:dev"),
        kind: "workspace.development".into(),
        recipe: None,
        task_ids: vec![TaskId::from("task:test")],
        goal_ids: vec![],
        relevant_views: vec![],
        baseline_capability_requirements: vec![capability.clone()],
        grammar: grammar(),
        experience_profiles: vec![],
        policy_overlays: vec![],
    };

    let task = Task {
        id: TaskId::from("task:test"),
        title: "Run tests".into(),
        state: TaskState::Ready,
        input_objects: vec![],
        requirement_graphs: vec![graph_id],
        output_objects: vec![],
        requested_effects: EffectEnvelope::default(),
    };

    let implementation_a = CapabilityImplementation {
        id: ImplementationId::from("impl:test-a"),
        implements: capability.clone(),
        runtime: RuntimeKind::Wasm,
        supported_platforms: vec!["linux-aarch64".into()],
        required_memory_bytes: 64 * 1024 * 1024,
        required_accelerators: vec![],
        network_required: false,
    };

    let implementation_b = CapabilityImplementation {
        id: ImplementationId::from("impl:test-b"),
        implements: capability.clone(),
        runtime: RuntimeKind::Oci,
        supported_platforms: vec!["linux-x86_64".into()],
        required_memory_bytes: 128 * 1024 * 1024,
        required_accelerators: vec![],
        network_required: false,
    };

    assert_eq!(workspace.baseline_capability_requirements, vec![capability.clone()]);
    assert_eq!(task.requirement_graphs, vec![RequirementGraphId::from("rg:test-1")]);
    assert_eq!(implementation_a.implements, capability);
    assert_eq!(implementation_b.implements, implementation_a.implements);
}

#[test]
fn same_surface_semantics_can_materialize_with_different_experience_profiles() {
    let surface = Surface {
        id: SurfaceId::from("surface:editor"),
        workspace: WorkspaceId::from("ws:dev"),
        task: Some(TaskId::from("task:edit")),
        role: SurfaceRole::Editor,
        persistence: SurfacePersistence::Persistent,
        semantic_state_ref: Some("object:source/main.rs".into()),
    };

    let mac = SurfaceMaterialization {
        surface: surface.id.clone(),
        profile: ExperienceProfileId::from("profile:macos-native"),
        platform_target: "aarch64-darwin".into(),
        renderer_instance: None,
    };

    let linux = SurfaceMaterialization {
        surface: surface.id.clone(),
        profile: ExperienceProfileId::from("profile:hyprland"),
        platform_target: "x86_64-linux".into(),
        renderer_instance: None,
    };

    assert_eq!(mac.surface, linux.surface);
    assert_ne!(mac.profile, linux.profile);
    assert_ne!(mac.platform_target, linux.platform_target);
}

#[test]
fn late_binding_is_scoped_by_a_lease_boundary() {
    let lease = BindingLease {
        id: BindingLeaseId::from("lease:test"),
        plan: BindingPlanId::from("plan:test"),
        valid_until_unix_ms: None,
        rebind_boundary: RebindBoundary::AtTaskCheckpoint,
        grants: vec![],
    };

    assert_eq!(lease.rebind_boundary, RebindBoundary::AtTaskCheckpoint);
}

#[test]
fn improvement_proposal_keeps_local_and_external_evidence_separate() {
    let proposal = ImprovementProposal {
        id: ImprovementProposalId::from("proposal:battery-kernel"),
        title: "Test upstream battery fix".into(),
        triggering_situations: vec![SituationId::from("situation:battery-regression")],
        local_evidence: vec!["idle discharge increased after system revision".into()],
        external_evidence: vec![ExternalEvidence {
            source_kind: "kernel-release-note".into(),
            title: "Upstream power-management fix".into(),
            canonical_reference: "official://kernel/release-note".into(),
            official_or_primary: true,
        }],
        applicability_reasoning: "The changed subsystem matches the observed hardware path".into(),
        proposed_changes: vec!["build candidate kernel generation".into()],
        expected_benefits: vec!["lower idle discharge".into()],
        risks: vec!["possible driver regression".into()],
        test_plan: vec!["A/B battery benchmark".into()],
        rollback_reference: Some("system:known-good".into()),
        required_autonomy: TechnicianAutonomy::Prepare,
        expires_at_unix_ms: None,
    };

    assert!(proposal.external_evidence[0].official_or_primary);
    assert!(!proposal.local_evidence.is_empty());
    assert_eq!(proposal.required_autonomy, TechnicianAutonomy::Prepare);
}
