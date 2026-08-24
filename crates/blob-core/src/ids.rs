use core::fmt;

macro_rules! define_id {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name(String);

            impl $name {
                pub fn new(value: impl Into<String>) -> Self {
                    Self(value.into())
                }

                pub fn as_str(&self) -> &str {
                    &self.0
                }
            }

            impl From<&str> for $name {
                fn from(value: &str) -> Self {
                    Self::new(value)
                }
            }

            impl From<String> for $name {
                fn from(value: String) -> Self {
                    Self::new(value)
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str(&self.0)
                }
            }
        )+
    };
}

define_id!(
    PersonalWorldId,
    GoalId,
    WorkspaceId,
    WorkspaceRecipeId,
    TaskId,
    RequirementGraphId,
    RequirementRoleId,
    CapabilityId,
    ImplementationId,
    NodeId,
    BindingPlanId,
    BindingLeaseId,
    KnowledgeObjectId,
    SurfaceId,
    EventId,
    SituationId,
    ImprovementProposalId,
    ExperienceProfileId,
    RepresentationId,
    ProjectionId,
    CausalRecordId,
);
