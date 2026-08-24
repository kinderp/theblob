use crate::ids::{
    CapabilityId, EventId, ImprovementProposalId, KnowledgeObjectId, NodeId, SituationId, TaskId,
    WorkspaceId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventSource {
    Kernel,
    FileSystem,
    Device,
    Network,
    Service,
    Capability,
    User,
    ExternalSource,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventTrustClass {
    LocalKernel,
    TrustedLocalService,
    TrustedFabricNode,
    SignedUpstreamMetadata,
    UntrustedExternal,
    UserDeclared,
    AiInferred,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubjectRef {
    Workspace(WorkspaceId),
    Task(TaskId),
    KnowledgeObject(KnowledgeObjectId),
    Node(NodeId),
    Capability(CapabilityId),
    PlatformSubject(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub id: EventId,
    pub schema_version: u16,
    pub source: EventSource,
    pub source_node: Option<NodeId>,
    pub kind: String,
    pub subjects: Vec<SubjectRef>,
    pub observed_at_unix_ms: u64,
    pub received_at_unix_ms: u64,
    pub source_sequence: Option<u64>,
    pub correlation_keys: Vec<String>,
    pub attributes: Vec<(String, String)>,
    pub trust: EventTrustClass,
    pub provenance: Vec<String>,
    pub causal_parent_ids: Vec<EventId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SituationWindow {
    pub start_unix_ms: u64,
    pub end_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Situation {
    pub id: SituationId,
    pub kind: String,
    pub summary: String,
    pub evidence_event_ids: Vec<EventId>,
    pub derived_facts: Vec<(String, String)>,
    pub subjects: Vec<SubjectRef>,
    pub window: SituationWindow,
    pub confidence_ppm: Option<u32>,
    pub deterministic_rule_provenance: Vec<String>,
    pub semantic_provenance: Vec<String>,
    pub expires_at_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TechnicianAutonomy {
    Observe,
    Suggest,
    Prepare,
    ApplyWithinPolicy,
    Forbidden,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReasoningTier {
    ResidentLocal,
    StrongLocal,
    TrustedFabric,
    CloudAllowed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiRoutingPolicy {
    pub allowed_tiers: Vec<ReasoningTier>,
    pub local_first: bool,
    pub max_cost_microeur: Option<u64>,
    pub require_local_for_sensitive_data: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalEvidence {
    pub source_kind: String,
    pub title: String,
    pub canonical_reference: String,
    pub official_or_primary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImprovementProposal {
    pub id: ImprovementProposalId,
    pub title: String,
    pub triggering_situations: Vec<SituationId>,
    pub local_evidence: Vec<String>,
    pub external_evidence: Vec<ExternalEvidence>,
    pub applicability_reasoning: String,
    pub proposed_changes: Vec<String>,
    pub expected_benefits: Vec<String>,
    pub risks: Vec<String>,
    pub test_plan: Vec<String>,
    pub rollback_reference: Option<String>,
    pub required_autonomy: TechnicianAutonomy,
    pub expires_at_unix_ms: Option<u64>,
}
