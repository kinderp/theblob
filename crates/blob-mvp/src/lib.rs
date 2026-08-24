#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::path::PathBuf;

use blob_alfred::{Alfred, SourceChangeRequiresTestRule};
use blob_core::{
    BindingLease, BindingPlan, CapabilityId, CausalKind, CausalRecord, CausalRecordId, EffectEnvelope,
    EffectKind, EffectSpec, Event, ImplementationId, RequirementGraph, RequirementGraphId,
    RequirementRole, RequirementRoleId, RequirementRoleKind, Situation, Task, TaskId, TaskState,
    WorkspaceId,
};
use blob_executor::{
    ExecutionError, ExecutionRequest, ExecutionResult, ExecutionStatus, LocalProcessCapsule,
    LocalProcessExecutor,
};
use blob_history::{HistoryError, InMemoryCausalLog};
use blob_resolver::{Registry, ResolveError, Resolver};

#[derive(Debug)]
pub enum MvpError {
    NoSituation,
    Resolve(ResolveError),
    MissingCapsule(ImplementationId),
    Execute(ExecutionError),
    History(HistoryError),
}

impl From<ResolveError> for MvpError {
    fn from(value: ResolveError) -> Self {
        Self::Resolve(value)
    }
}

impl From<ExecutionError> for MvpError {
    fn from(value: ExecutionError) -> Self {
        Self::Execute(value)
    }
}

impl From<HistoryError> for MvpError {
    fn from(value: HistoryError) -> Self {
        Self::History(value)
    }
}

#[derive(Clone, Debug)]
pub struct VerticalSliceOutcome {
    pub situation: Situation,
    pub task: Task,
    pub graph: RequirementGraph,
    pub plan: BindingPlan,
    pub lease: BindingLease,
    pub execution: ExecutionResult,
    pub causal_records: Vec<CausalRecord>,
}

pub fn run_source_change_test(
    event: Event,
    active_workspace: WorkspaceId,
    registry: &Registry,
    process_capsules: &HashMap<ImplementationId, LocalProcessCapsule>,
    working_directory: PathBuf,
) -> Result<VerticalSliceOutcome, MvpError> {
    let mut history = InMemoryCausalLog::default();

    let event_cause = CausalRecordId::new(format!("cause:event:{}", event.id));
    history.append(CausalRecord {
        id: event_cause.clone(),
        kind: CausalKind::EventObserved,
        occurred_at_unix_ms: event.observed_at_unix_ms,
        parents: vec![],
        actor: "alfred".into(),
        summary: format!("Observed {}", event.kind),
        why: "normalized event entered Alfred".into(),
        event: Some(event.id.clone()),
        situation: None,
        task: None,
        requirement_graph: None,
        binding_plan: None,
        binding_lease: None,
        improvement_proposal: None,
        evidence: event.provenance.clone(),
        expected_effects: vec![],
        actual_effects: vec![],
        authorization: None,
        rollback_reference: None,
    })?;

    let mut alfred = Alfred::new(vec![Box::new(SourceChangeRequiresTestRule::v1(
        active_workspace,
    ))]);
    let ingest = alfred.ingest(event.clone());
    let situation = ingest
        .situations
        .into_iter()
        .next()
        .ok_or(MvpError::NoSituation)?;

    let situation_cause = CausalRecordId::new(format!("cause:situation:{}", situation.id));
    history.append(CausalRecord {
        id: situation_cause.clone(),
        kind: CausalKind::SituationDerived,
        occurred_at_unix_ms: situation.window.end_unix_ms,
        parents: vec![event_cause],
        actor: "alfred".into(),
        summary: situation.summary.clone(),
        why: "deterministic correlation rule matched source change".into(),
        event: None,
        situation: Some(situation.id.clone()),
        task: None,
        requirement_graph: None,
        binding_plan: None,
        binding_lease: None,
        improvement_proposal: None,
        evidence: situation
            .evidence_event_ids
            .iter()
            .map(ToString::to_string)
            .collect(),
        expected_effects: vec![],
        actual_effects: vec![],
        authorization: None,
        rollback_reference: None,
    })?;

    let mut task = Task {
        id: TaskId::new(format!("task:test:{}", event.id)),
        title: "Run tests after relevant source change".into(),
        state: TaskState::Ready,
        input_objects: vec![],
        requirement_graphs: vec![],
        output_objects: vec![],
        requested_effects: EffectEnvelope {
            effects: vec![EffectSpec {
                kind: EffectKind::Execute,
                target_type: None,
                reversible: true,
            }],
        },
    };

    let graph = RequirementGraph {
        id: RequirementGraphId::new(format!("rg:{}", task.id)),
        roles: vec![RequirementRole {
            id: RequirementRoleId::new(format!("role:test-runner:{}", task.id)),
            kind: RequirementRoleKind::Capability(CapabilityId::from("test.run")),
        }],
        relations: vec![],
        constraints: vec![],
        requested_effects: task.requested_effects.clone(),
    };
    task.requirement_graphs.push(graph.id.clone());

    let task_cause = CausalRecordId::new(format!("cause:task:{}", task.id));
    history.append(CausalRecord {
        id: task_cause.clone(),
        kind: CausalKind::TaskTransition,
        occurred_at_unix_ms: event.received_at_unix_ms,
        parents: vec![situation_cause],
        actor: "development-workspace".into(),
        summary: "Created test Task and RequirementGraph".into(),
        why: "source-change Situation requires tests to be reconsidered".into(),
        event: None,
        situation: Some(situation.id.clone()),
        task: Some(task.id.clone()),
        requirement_graph: Some(graph.id.clone()),
        binding_plan: None,
        binding_lease: None,
        improvement_proposal: None,
        evidence: vec![],
        expected_effects: vec!["execute test.run".into()],
        actual_effects: vec![],
        authorization: Some("workspace deterministic test policy".into()),
        rollback_reference: None,
    })?;

    let (plan, lease) = Resolver::new(registry).resolve_single_capability(&graph)?;
    let binding_cause = CausalRecordId::new(format!("cause:binding:{}", plan.id));
    history.append(CausalRecord {
        id: binding_cause.clone(),
        kind: CausalKind::BindingResolved,
        occurred_at_unix_ms: event.received_at_unix_ms,
        parents: vec![task_cause],
        actor: "blob-resolver".into(),
        summary: "Resolved and independently verified test.run binding".into(),
        why: plan
            .trace
            .tie_break_note
            .clone()
            .unwrap_or_else(|| "deterministic candidate resolution".into()),
        event: None,
        situation: Some(situation.id.clone()),
        task: Some(task.id.clone()),
        requirement_graph: Some(graph.id.clone()),
        binding_plan: Some(plan.id.clone()),
        binding_lease: Some(lease.id.clone()),
        improvement_proposal: None,
        evidence: plan.trace.candidate_notes.clone(),
        expected_effects: vec!["execute selected test implementation".into()],
        actual_effects: vec![],
        authorization: Some("BindingVerifier accepted plan".into()),
        rollback_reference: None,
    })?;

    let selected = plan
        .resolved_capabilities
        .first()
        .expect("MVP resolver always returns one resolved capability");
    let capsule = process_capsules
        .get(&selected.implementation)
        .cloned()
        .ok_or_else(|| MvpError::MissingCapsule(selected.implementation.clone()))?;

    task.state = TaskState::Running;
    let execution = LocalProcessExecutor::execute(&ExecutionRequest {
        task: task.id.clone(),
        lease: lease.id.clone(),
        working_directory,
        capsule,
    })?;

    task.state = match execution.status {
        ExecutionStatus::Succeeded => TaskState::Completed,
        ExecutionStatus::Failed => TaskState::Failed,
    };

    let execution_cause = CausalRecordId::new(format!("cause:execution:{}", task.id));
    history.append(CausalRecord {
        id: execution_cause,
        kind: CausalKind::ExecutionCompleted,
        occurred_at_unix_ms: event.received_at_unix_ms,
        parents: vec![binding_cause],
        actor: "blob-executor".into(),
        summary: format!("test.run finished with {:?}", execution.status),
        why: "verified BindingLease authorized the selected implementation".into(),
        event: None,
        situation: Some(situation.id.clone()),
        task: Some(task.id.clone()),
        requirement_graph: Some(graph.id.clone()),
        binding_plan: Some(plan.id.clone()),
        binding_lease: Some(lease.id.clone()),
        improvement_proposal: None,
        evidence: vec![format!("exit_code={:?}", execution.exit_code)],
        expected_effects: vec!["run tests".into()],
        actual_effects: vec![format!("task_state={:?}", task.state)],
        authorization: Some("scoped BindingLease".into()),
        rollback_reference: None,
    })?;

    Ok(VerticalSliceOutcome {
        situation,
        task,
        graph,
        plan,
        lease,
        execution,
        causal_records: history.records().to_vec(),
    })
}
