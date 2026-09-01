#![forbid(unsafe_code)]

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurfaceRoleId(String);

impl SurfaceRoleId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SurfaceRoleId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for SurfaceRoleId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspacePresentationMode {
    Blob,
    Tile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceSlot {
    pub column: u8,
    pub row: u8,
    pub column_span: u8,
    pub row_span: u8,
}

impl SurfaceSlot {
    pub const fn new(column: u8, row: u8, column_span: u8, row_span: u8) -> Self {
        Self {
            column,
            row,
            column_span,
            row_span,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceInstanceState {
    pub id: String,
    pub surface_id: String,
    pub workspace_id: String,
    pub role: SurfaceRoleId,
    pub host_id: String,
    pub slot: SurfaceSlot,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceShellState {
    pub id: String,
    pub title: String,
    pub presentation: WorkspacePresentationMode,
    pub surface_instances: Vec<SurfaceInstanceState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobShellState {
    focused_workspace: usize,
    pub workspaces: Vec<WorkspaceShellState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobShellEffect {
    WorkspaceFocused(usize),
    WorkspacePresentationChanged {
        workspace: usize,
        presentation: WorkspacePresentationMode,
    },
    WorkspacesCollapsed,
}

impl BlobShellState {
    /// Small local topology used by the P0 shell demo.
    ///
    /// The model is renderer-neutral: Slint receives workspace/surface state but
    /// does not define Workspace identity, Surface roles or placement semantics.
    /// All SurfaceInstances are deliberately local in P0; remote Fabric placement
    /// is a later slice.
    pub fn demo_local(host_id: impl Into<String>) -> Self {
        let host_id = host_id.into();

        let romeo = WorkspaceShellState {
            id: "workspace:romeo".into(),
            title: "Romeo".into(),
            presentation: WorkspacePresentationMode::Blob,
            surface_instances: vec![
                instance(
                    "romeo:editor:local",
                    "surface:romeo:editor",
                    "workspace:romeo",
                    "code.editor",
                    &host_id,
                    SurfaceSlot::new(0, 0, 1, 1),
                ),
                instance(
                    "romeo:docs:local",
                    "surface:romeo:docs",
                    "workspace:romeo",
                    "docs.context",
                    &host_id,
                    SurfaceSlot::new(1, 0, 1, 1),
                ),
                instance(
                    "romeo:terminal:local",
                    "surface:romeo:terminal",
                    "workspace:romeo",
                    "terminal.session",
                    &host_id,
                    SurfaceSlot::new(0, 1, 1, 1),
                ),
                instance(
                    "romeo:tests:local",
                    "surface:romeo:tests",
                    "workspace:romeo",
                    "test.status",
                    &host_id,
                    SurfaceSlot::new(1, 1, 1, 1),
                ),
            ],
        };

        let docs = WorkspaceShellState {
            id: "workspace:docs".into(),
            title: "Docs".into(),
            presentation: WorkspacePresentationMode::Blob,
            surface_instances: vec![
                instance(
                    "docs:reader:local",
                    "surface:docs:reader",
                    "workspace:docs",
                    "document.reader",
                    &host_id,
                    SurfaceSlot::new(0, 0, 2, 1),
                ),
                instance(
                    "docs:related:local",
                    "surface:docs:related",
                    "workspace:docs",
                    "document.related",
                    &host_id,
                    SurfaceSlot::new(0, 1, 2, 1),
                ),
            ],
        };

        let system = WorkspaceShellState {
            id: "workspace:system".into(),
            title: "System".into(),
            presentation: WorkspacePresentationMode::Blob,
            surface_instances: vec![
                instance(
                    "system:health:local",
                    "surface:system:health",
                    "workspace:system",
                    "system.health",
                    &host_id,
                    SurfaceSlot::new(0, 0, 1, 1),
                ),
                instance(
                    "system:controls:local",
                    "surface:system:controls",
                    "workspace:system",
                    "system.controls",
                    &host_id,
                    SurfaceSlot::new(1, 0, 1, 1),
                ),
            ],
        };

        let notes = WorkspaceShellState {
            id: "workspace:notes".into(),
            title: "Notes".into(),
            presentation: WorkspacePresentationMode::Blob,
            surface_instances: vec![instance(
                "notes:scratch:local",
                "surface:notes:scratch",
                "workspace:notes",
                "notes.scratchpad",
                &host_id,
                SurfaceSlot::new(0, 0, 2, 2),
            )],
        };

        Self {
            focused_workspace: 0,
            workspaces: vec![romeo, docs, system, notes],
        }
    }

    pub fn focused_workspace_index(&self) -> usize {
        self.focused_workspace
    }

    pub fn focused_workspace(&self) -> &WorkspaceShellState {
        &self.workspaces[self.focused_workspace]
    }

    pub fn expanded_workspace_index(&self) -> Option<usize> {
        self.workspaces
            .iter()
            .position(|workspace| workspace.presentation == WorkspacePresentationMode::Tile)
    }

    pub fn focus(&mut self, workspace: usize) -> Option<BlobShellEffect> {
        if workspace >= self.workspaces.len() {
            return None;
        }
        self.focused_workspace = workspace;
        Some(BlobShellEffect::WorkspaceFocused(workspace))
    }

    /// P0 shell policy: one Workspace can be unfolded at a time.
    ///
    /// This is intentionally a shell policy rather than a limitation of the
    /// SurfaceInstance model. Later layout modes may show multiple Workspace
    /// compositions simultaneously.
    pub fn toggle_single_workspace(&mut self, workspace: usize) -> Option<BlobShellEffect> {
        if workspace >= self.workspaces.len() {
            return None;
        }

        self.focused_workspace = workspace;
        let next = if self.workspaces[workspace].presentation == WorkspacePresentationMode::Tile {
            WorkspacePresentationMode::Blob
        } else {
            WorkspacePresentationMode::Tile
        };

        for item in &mut self.workspaces {
            item.presentation = WorkspacePresentationMode::Blob;
        }
        self.workspaces[workspace].presentation = next;

        Some(BlobShellEffect::WorkspacePresentationChanged {
            workspace,
            presentation: next,
        })
    }

    pub fn toggle_focused_workspace(&mut self) -> BlobShellEffect {
        self.toggle_single_workspace(self.focused_workspace)
            .expect("focused workspace index must always be valid")
    }

    pub fn collapse_all(&mut self) -> BlobShellEffect {
        for workspace in &mut self.workspaces {
            workspace.presentation = WorkspacePresentationMode::Blob;
        }
        BlobShellEffect::WorkspacesCollapsed
    }
}

fn instance(
    id: &str,
    surface_id: &str,
    workspace_id: &str,
    role: &str,
    host_id: &str,
    slot: SurfaceSlot,
) -> SurfaceInstanceState {
    SurfaceInstanceState {
        id: id.into(),
        surface_id: surface_id.into(),
        workspace_id: workspace_id.into(),
        role: SurfaceRoleId::from(role),
        host_id: host_id.into(),
        slot,
        visible: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_demo_state_has_four_workspaces_and_renderer_neutral_local_surfaces() {
        let state = BlobShellState::demo_local("host:test");
        assert_eq!(state.workspaces.len(), 4);
        assert_eq!(state.workspaces[0].title, "Romeo");
        assert_eq!(state.workspaces[0].surface_instances.len(), 4);
        assert!(state
            .workspaces
            .iter()
            .flat_map(|workspace| &workspace.surface_instances)
            .all(|instance| instance.host_id == "host:test"));
    }

    #[test]
    fn p0_toggle_changes_presentation_without_changing_surface_identity() {
        let mut state = BlobShellState::demo_local("host:test");
        let before = state.workspaces[0].surface_instances.clone();

        assert_eq!(
            state.toggle_single_workspace(0),
            Some(BlobShellEffect::WorkspacePresentationChanged {
                workspace: 0,
                presentation: WorkspacePresentationMode::Tile,
            })
        );
        assert_eq!(state.workspaces[0].surface_instances, before);
        assert_eq!(state.expanded_workspace_index(), Some(0));

        state.toggle_single_workspace(0);
        assert_eq!(state.expanded_workspace_index(), None);
    }

    #[test]
    fn p0_policy_keeps_only_one_workspace_unfolded() {
        let mut state = BlobShellState::demo_local("host:test");
        state.toggle_single_workspace(0);
        state.toggle_single_workspace(2);

        assert_eq!(state.expanded_workspace_index(), Some(2));
        assert_eq!(
            state.workspaces[0].presentation,
            WorkspacePresentationMode::Blob
        );
        assert_eq!(state.focused_workspace_index(), 2);
    }
}
