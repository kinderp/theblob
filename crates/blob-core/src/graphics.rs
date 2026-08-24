use crate::ids::{ExperienceProfileId, SurfaceId, TaskId, WorkspaceId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RendererFamily {
    BlobNative,
    MacOsNative,
    HyprlandWayland,
    AndroidNative,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExperienceProfile {
    pub id: ExperienceProfileId,
    pub renderer_family: RendererFamily,
    pub visual_style: String,
    pub keyboard_policy: String,
    pub gesture_policy: String,
    pub animation_policy: String,
    pub density: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurfacePersistence {
    Persistent,
    Session,
    Contextual,
    Ephemeral,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurfaceRole {
    ObjectNavigator,
    Editor,
    Terminal,
    TechnicianPanel,
    TaskStatus,
    Timeline,
    ComparisonPanel,
    Notification,
    LegacySurface,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Surface {
    pub id: SurfaceId,
    pub workspace: WorkspaceId,
    pub task: Option<TaskId>,
    pub role: SurfaceRole,
    pub persistence: SurfacePersistence,
    pub semantic_state_ref: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceMaterialization {
    pub surface: SurfaceId,
    pub profile: ExperienceProfileId,
    pub platform_target: String,
    pub renderer_instance: Option<String>,
}
