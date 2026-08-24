use crate::ids::{
    BindingLeaseId, BindingPlanId, CapabilityId, ImplementationId, KnowledgeObjectId, NodeId,
    RequirementGraphId, RequirementRoleId, TaskId,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticType(String);

impl SemanticType {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SemanticType {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Read,
    Write,
    Create,
    Delete,
    Network,
    Execute,
    Notify,
    Send,
    Purchase,
    SystemChange,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectSpec {
    pub kind: EffectKind,
    pub target_type: Option<SemanticType>,
    pub reversible: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EffectEnvelope {
    pub effects: Vec<EffectSpec>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityContract {
    pub id: CapabilityId,
    pub inputs: Vec<SemanticType>,
    pub outputs: Vec<SemanticType>,
    pub effects: EffectEnvelope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeKind {
    Wasm,
    Oci,
    MicroVm,
    Native,
    LocalModel,
    RemoteService,
    Hardware,
}

impl RuntimeKind {
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Wasm => "wasm",
            Self::Oci => "oci",
            Self::MicroVm => "microvm",
            Self::Native => "native",
            Self::LocalModel => "local-model",
            Self::RemoteService => "remote-service",
            Self::Hardware => "hardware",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityImplementation {
    pub id: ImplementationId,
    pub implements: CapabilityId,
    pub runtime: RuntimeKind,
    pub trusted: bool,
    pub supported_platforms: Vec<String>,
    pub required_memory_bytes: u64,
    pub required_accelerators: Vec<String>,
    pub network_required: bool,
    pub quality_ppm: u32,
    pub expected_latency_us: u64,
    pub expected_energy_uj: u64,
    pub cost_microeur: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeFacts {
    pub id: NodeId,
    pub platform: String,
    pub trusted: bool,
    pub online: bool,
    pub memory_bytes: u64,
    pub accelerator_tags: Vec<String>,
    pub runtime_tags: Vec<String>,
    pub data_residency: String,
    pub control_depth: ControlDepth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlDepth {
    Hosted,
    UserlandMutable,
    SystemMutable,
    KernelMutable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequirementRoleKind {
    Capability(CapabilityId),
    ExistingObject(KnowledgeObjectId),
    DesiredOutput(SemanticType),
    ComputeNode,
    Resource(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequirementRole {
    pub id: RequirementRoleId,
    pub kind: RequirementRoleKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelationKind {
    Consumes,
    Produces,
    Uses,
    ExecutesOn,
    ConvertsTo,
    DependsOn,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequirementRelation {
    pub from: RequirementRoleId,
    pub kind: RelationKind,
    pub to: RequirementRoleId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintClass {
    Policy,
    Hard,
    Preference,
    Objective,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintValue {
    Bool(bool),
    Int(i64),
    Text(String),
    StableId(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintExpr {
    Eq(String, ConstraintValue),
    Ne(String, ConstraintValue),
    Lt(String, i64),
    Le(String, i64),
    Gt(String, i64),
    Ge(String, i64),
    In(String, Vec<ConstraintValue>),
    And(Vec<ConstraintExpr>),
    Or(Vec<ConstraintExpr>),
    Not(Box<ConstraintExpr>),
    Implies(Box<ConstraintExpr>, Box<ConstraintExpr>),
    ExactlyOne(Vec<String>),
    AtMost { count: usize, symbols: Vec<String> },
    AtLeast { count: usize, symbols: Vec<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint {
    pub class: ConstraintClass,
    pub expression: ConstraintExpr,
    pub explanation: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequirementGraph {
    pub id: RequirementGraphId,
    pub roles: Vec<RequirementRole>,
    pub relations: Vec<RequirementRelation>,
    pub constraints: Vec<Constraint>,
    pub requested_effects: EffectEnvelope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskState {
    Planned,
    Ready,
    Running,
    WaitingForApproval,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub state: TaskState,
    pub input_objects: Vec<KnowledgeObjectId>,
    pub requirement_graphs: Vec<RequirementGraphId>,
    pub output_objects: Vec<KnowledgeObjectId>,
    pub requested_effects: EffectEnvelope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedCapabilityRole {
    pub role: RequirementRoleId,
    pub capability: CapabilityId,
    pub implementation: ImplementationId,
    pub node: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegatedGrant {
    pub capability: String,
    pub scope: String,
    pub expires_at_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataRoute {
    pub source_role: RequirementRoleId,
    pub destination_role: RequirementRoleId,
    pub projection: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolutionTrace {
    pub candidate_notes: Vec<String>,
    pub rejected_candidates: Vec<String>,
    pub solver_backend: Option<String>,
    pub verifier_notes: Vec<String>,
    pub objective_vector: Vec<i64>,
    pub tie_break_note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingPlan {
    pub id: BindingPlanId,
    pub graph: RequirementGraphId,
    pub resolved_capabilities: Vec<ResolvedCapabilityRole>,
    pub grants: Vec<DelegatedGrant>,
    pub data_routes: Vec<DataRoute>,
    pub expected_effects: EffectEnvelope,
    pub trace: ResolutionTrace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RebindBoundary {
    Never,
    BeforeExecution,
    AtTaskCheckpoint,
    BetweenPureSteps,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingLease {
    pub id: BindingLeaseId,
    pub plan: BindingPlanId,
    pub valid_until_unix_ms: Option<u64>,
    pub rebind_boundary: RebindBoundary,
    pub grants: Vec<DelegatedGrant>,
}
