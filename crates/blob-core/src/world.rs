use crate::execution::SemanticType;
use crate::ids::{
    CapabilityId, ExperienceProfileId, GoalId, KnowledgeObjectId, NodeId, PersonalWorldId,
    ProjectionId, RepresentationId, TaskId, WorkspaceId, WorkspaceRecipeId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonalWorld {
    pub id: PersonalWorldId,
    pub workspace_ids: Vec<WorkspaceId>,
    pub goal_ids: Vec<GoalId>,
    pub trusted_node_ids: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Goal {
    pub id: GoalId,
    pub title: String,
    pub success_criteria: Vec<String>,
    pub task_ids: Vec<TaskId>,
    pub workspace_ids: Vec<WorkspaceId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExperienceGrammar {
    pub semantic_roles: Vec<String>,
    pub navigation_model: String,
    pub command_model: String,
    pub stable_shortcuts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceConstructionMode {
    Ready,
    AiDesigned,
    Expert,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceRecipe {
    pub id: WorkspaceRecipeId,
    pub kind: String,
    pub version: String,
    pub construction_mode: WorkspaceConstructionMode,
    pub grammar: ExperienceGrammar,
    pub baseline_capabilities: Vec<CapabilityId>,
    pub policy_defaults: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceExperienceBinding {
    pub device_selector: String,
    pub profile: ExperienceProfileId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub kind: String,
    pub recipe: Option<WorkspaceRecipeId>,
    pub task_ids: Vec<TaskId>,
    pub goal_ids: Vec<GoalId>,
    pub relevant_views: Vec<String>,
    pub baseline_capability_requirements: Vec<CapabilityId>,
    pub grammar: ExperienceGrammar,
    pub experience_profiles: Vec<WorkspaceExperienceBinding>,
    pub policy_overlays: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeObject {
    pub id: KnowledgeObjectId,
    pub semantic_type: SemanticType,
    pub title: String,
    pub content_ref: String,
    pub metadata: Vec<(String, String)>,
    pub relations: Vec<ObjectRelation>,
    pub provenance: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectRelation {
    pub relation: String,
    pub target: KnowledgeObjectId,
    pub confidence_ppm: Option<u32>,
    pub provenance: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projection {
    pub id: ProjectionId,
    pub source: KnowledgeObjectId,
    pub projection_type: SemanticType,
    pub selector: String,
    pub allowed_fields: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepresentationFreshness {
    Fresh,
    Stale,
    Absent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Representation {
    pub id: RepresentationId,
    pub source: KnowledgeObjectId,
    pub representation_type: SemanticType,
    pub transformation_capability: CapabilityId,
    pub source_revision: String,
    pub artifact_ref: Option<String>,
    pub freshness: RepresentationFreshness,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub id: String,
    pub title: String,
    pub query: String,
    pub object_ids: Vec<KnowledgeObjectId>,
}
