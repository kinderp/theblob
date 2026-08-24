use crate::ids::{EventId, ImprovementProposalId, SituationId};

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
pub struct Event {
    pub id: EventId,
    pub source: EventSource,
    pub kind: String,
    pub occurred_at_unix_ms: u64,
    pub attributes: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Situation {
    pub id: SituationId,
    pub kind: String,
    pub summary: String,
    pub evidence_event_ids: Vec<EventId>,
    pub confidence_ppm: Option<u32>,
    pub semantic_provenance: Vec<String>,
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
