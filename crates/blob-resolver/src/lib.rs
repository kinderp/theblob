#![forbid(unsafe_code)]

use std::cmp::Ordering;

use blob_core::{
    BindingLease, BindingLeaseId, BindingPlan, BindingPlanId, CapabilityImplementation,
    CapabilityId, NodeFacts, RebindBoundary, RequirementGraph, RequirementRoleKind,
    ResolvedCapabilityRole, ResolutionTrace,
};

#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub implementations: Vec<CapabilityImplementation>,
    pub nodes: Vec<NodeFacts>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolveError {
    UnsupportedGraph(String),
    NoCandidate,
    VerificationFailed(Vec<String>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VerificationReport {
    pub errors: Vec<String>,
}

impl VerificationReport {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

pub struct Resolver<'a> {
    registry: &'a Registry,
}

impl<'a> Resolver<'a> {
    pub fn new(registry: &'a Registry) -> Self {
        Self { registry }
    }

    pub fn resolve_single_capability(
        &self,
        graph: &RequirementGraph,
    ) -> Result<(BindingPlan, BindingLease), ResolveError> {
        if !graph.constraints.is_empty() {
            return Err(ResolveError::UnsupportedGraph(
                "MVP resolver does not evaluate Constraint IR yet".into(),
            ));
        }

        let capability_roles: Vec<_> = graph
            .roles
            .iter()
            .filter_map(|role| match &role.kind {
                RequirementRoleKind::Capability(capability) => {
                    Some((role.id.clone(), capability.clone()))
                }
                _ => None,
            })
            .collect();

        if capability_roles.len() != 1 {
            return Err(ResolveError::UnsupportedGraph(
                "MVP resolver requires exactly one Capability role".into(),
            ));
        }

        let (role_id, capability_id) = &capability_roles[0];
        let mut valid = Vec::new();
        let mut rejected = Vec::new();

        for implementation in self
            .registry
            .implementations
            .iter()
            .filter(|implementation| &implementation.implements == capability_id)
        {
            for node in &self.registry.nodes {
                let reasons = candidate_rejection_reasons(implementation, node);
                if reasons.is_empty() {
                    valid.push((implementation, node));
                } else {
                    rejected.push(format!(
                        "{}@{} rejected: {}",
                        implementation.id,
                        node.id,
                        reasons.join(", ")
                    ));
                }
            }
        }

        valid.sort_by(|(impl_a, node_a), (impl_b, node_b)| {
            compare_candidates(impl_a, node_a, impl_b, node_b)
        });

        let Some((implementation, node)) = valid.first().copied() else {
            return Err(ResolveError::NoCandidate);
        };

        let plan_id = BindingPlanId::new(format!(
            "plan:{}:{}:{}",
            graph.id,
            implementation.id,
            node.id
        ));

        let mut trace = ResolutionTrace {
            candidate_notes: valid
                .iter()
                .map(|(candidate_impl, candidate_node)| {
                    format!("valid: {}@{}", candidate_impl.id, candidate_node.id)
                })
                .collect(),
            rejected_candidates: rejected,
            solver_backend: Some("rust-mvp-deterministic@v1".into()),
            verifier_notes: Vec::new(),
            objective_vector: vec![
                implementation.quality_ppm as i64,
                implementation.cost_microeur as i64,
                implementation.expected_latency_us as i64,
                implementation.expected_energy_uj as i64,
            ],
            tie_break_note: Some("quality desc, cost/latency/energy asc, stable IDs".into()),
        };

        let mut plan = BindingPlan {
            id: plan_id.clone(),
            graph: graph.id.clone(),
            resolved_capabilities: vec![ResolvedCapabilityRole {
                role: role_id.clone(),
                capability: capability_id.clone(),
                implementation: implementation.id.clone(),
                node: node.id.clone(),
            }],
            grants: Vec::new(),
            data_routes: Vec::new(),
            expected_effects: graph.requested_effects.clone(),
            trace: trace.clone(),
        };

        let verification = BindingVerifier::verify(self.registry, graph, &plan);
        if !verification.is_valid() {
            return Err(ResolveError::VerificationFailed(verification.errors));
        }

        trace.verifier_notes.push("binding independently verified".into());
        plan.trace = trace;

        let lease = BindingLease {
            id: BindingLeaseId::new(format!("lease:{}", plan_id)),
            plan: plan_id,
            valid_until_unix_ms: None,
            rebind_boundary: RebindBoundary::BeforeExecution,
            grants: plan.grants.clone(),
        };

        Ok((plan, lease))
    }
}

pub struct BindingVerifier;

impl BindingVerifier {
    pub fn verify(
        registry: &Registry,
        graph: &RequirementGraph,
        plan: &BindingPlan,
    ) -> VerificationReport {
        let mut report = VerificationReport::default();

        if plan.graph != graph.id {
            report.errors.push("plan references a different RequirementGraph".into());
        }

        for resolved in &plan.resolved_capabilities {
            let requested_capability = graph.roles.iter().find_map(|role| {
                if role.id == resolved.role {
                    match &role.kind {
                        RequirementRoleKind::Capability(capability) => Some(capability),
                        _ => None,
                    }
                } else {
                    None
                }
            });

            match requested_capability {
                Some(capability) if capability == &resolved.capability => {}
                Some(_) => report
                    .errors
                    .push(format!("role {} resolved to wrong capability", resolved.role)),
                None => report
                    .errors
                    .push(format!("role {} is not a requested Capability role", resolved.role)),
            }

            let implementation = registry
                .implementations
                .iter()
                .find(|candidate| candidate.id == resolved.implementation);
            let node = registry.nodes.iter().find(|candidate| candidate.id == resolved.node);

            let Some(implementation) = implementation else {
                report
                    .errors
                    .push(format!("implementation {} not found", resolved.implementation));
                continue;
            };

            if implementation.implements != resolved.capability {
                report.errors.push(format!(
                    "implementation {} does not implement {}",
                    implementation.id, resolved.capability
                ));
            }

            let Some(node) = node else {
                report.errors.push(format!("node {} not found", resolved.node));
                continue;
            };

            report
                .errors
                .extend(candidate_rejection_reasons(implementation, node));
        }

        report
    }
}

fn candidate_rejection_reasons(
    implementation: &CapabilityImplementation,
    node: &NodeFacts,
) -> Vec<String> {
    let mut reasons = Vec::new();

    if !implementation.trusted {
        reasons.push("implementation is not trusted".into());
    }
    if !node.trusted {
        reasons.push("node is not trusted".into());
    }
    if !node.online {
        reasons.push("node is offline".into());
    }
    if !implementation
        .supported_platforms
        .iter()
        .any(|platform| platform == "*" || platform == &node.platform)
    {
        reasons.push("platform mismatch".into());
    }
    if node.memory_bytes < implementation.required_memory_bytes {
        reasons.push("insufficient memory".into());
    }
    if !implementation
        .required_accelerators
        .iter()
        .all(|required| node.accelerator_tags.iter().any(|available| available == required))
    {
        reasons.push("missing accelerator".into());
    }
    if !node
        .runtime_tags
        .iter()
        .any(|runtime| runtime == implementation.runtime.tag())
    {
        reasons.push("runtime unavailable".into());
    }

    reasons
}

fn compare_candidates(
    impl_a: &CapabilityImplementation,
    node_a: &NodeFacts,
    impl_b: &CapabilityImplementation,
    node_b: &NodeFacts,
) -> Ordering {
    impl_b
        .quality_ppm
        .cmp(&impl_a.quality_ppm)
        .then_with(|| impl_a.cost_microeur.cmp(&impl_b.cost_microeur))
        .then_with(|| impl_a.expected_latency_us.cmp(&impl_b.expected_latency_us))
        .then_with(|| impl_a.expected_energy_uj.cmp(&impl_b.expected_energy_uj))
        .then_with(|| impl_a.id.as_str().cmp(impl_b.id.as_str()))
        .then_with(|| node_a.id.as_str().cmp(node_b.id.as_str()))
}
