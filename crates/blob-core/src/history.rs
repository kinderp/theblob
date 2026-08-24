use crate::ids::{
    BindingLeaseId, BindingPlanId, CausalRecordId, EventId, ImprovementProposalId,
    RequirementGraphId, SituationId, TaskId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CausalKind {
    EventObserved,
    SituationDerived,
    TaskTransition,
    BindingResolved,
    ExecutionCompleted,
    ImprovementProposed,
    SystemChange,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CausalRecord {
    pub id: CausalRecordId,
    pub kind: CausalKind,
    pub occurred_at_unix_ms: u64,
    pub parents: Vec<CausalRecordId>,
    pub actor: String,
    pub summary: String,
    pub why: String,
    pub event: Option<EventId>,
    pub situation: Option<SituationId>,
    pub task: Option<TaskId>,
    pub requirement_graph: Option<RequirementGraphId>,
    pub binding_plan: Option<BindingPlanId>,
    pub binding_lease: Option<BindingLeaseId>,
    pub improvement_proposal: Option<ImprovementProposalId>,
    pub evidence: Vec<String>,
    pub expected_effects: Vec<String>,
    pub actual_effects: Vec<String>,
    pub authorization: Option<String>,
    pub rollback_reference: Option<String>,
}
