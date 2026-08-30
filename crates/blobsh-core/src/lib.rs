#![forbid(unsafe_code)]

pub mod grammar;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlobshDepth {
    User,
    Intent,
    Plan,
    Spec,
    Backend,
    Native,
}

impl BlobshDepth {
    pub const ALL: [Self; 6] = [
        Self::User,
        Self::Intent,
        Self::Plan,
        Self::Spec,
        Self::Backend,
        Self::Native,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "USER",
            Self::Intent => "INTENT",
            Self::Plan => "PLAN",
            Self::Spec => "SPEC",
            Self::Backend => "BACKEND",
            Self::Native => "NATIVE",
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::User => 0,
            Self::Intent => 1,
            Self::Plan => 2,
            Self::Spec => 3,
            Self::Backend => 4,
            Self::Native => 5,
        }
    }

    pub const fn shallower(self) -> Option<Self> {
        match self {
            Self::User => None,
            Self::Intent => Some(Self::User),
            Self::Plan => Some(Self::Intent),
            Self::Spec => Some(Self::Plan),
            Self::Backend => Some(Self::Spec),
            Self::Native => Some(Self::Backend),
        }
    }

    pub const fn deeper(self) -> Option<Self> {
        match self {
            Self::User => Some(Self::Intent),
            Self::Intent => Some(Self::Plan),
            Self::Plan => Some(Self::Spec),
            Self::Spec => Some(Self::Backend),
            Self::Backend => Some(Self::Native),
            Self::Native => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobshProvenance {
    UserInput,
    AiProposed,
    Derived,
    BackendGenerated,
    UserEdited,
    UserNativeOverride,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobshAssistanceMode {
    Quiet,
    Assist,
    Teach,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobshCommandStatus {
    Preview,
    NeedsRegeneration,
    Ready,
    ApprovalRequired,
    Invalid,
}

impl BlobshCommandStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::NeedsRegeneration => "needs regeneration",
            Self::Ready => "ready",
            Self::ApprovalRequired => "approval required",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobshDocKind {
    BlobSchema,
    ProjectDocs,
    PlatformDocs,
    ManPage,
    ExternalReference,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobshDocRef {
    pub kind: BlobshDocKind,
    pub title: String,
    pub target: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobshCommandLayer {
    pub depth: BlobshDepth,
    pub text: String,
    pub editable: bool,
    pub provenance: BlobshProvenance,
    pub explanation: Option<String>,
    pub documentation: Vec<BlobshDocRef>,
}

impl BlobshCommandLayer {
    pub fn new(
        depth: BlobshDepth,
        text: impl Into<String>,
        editable: bool,
        provenance: BlobshProvenance,
    ) -> Self {
        Self {
            depth,
            text: text.into(),
            editable,
            provenance,
            explanation: None,
            documentation: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobshCompletionItem {
    pub insert_text: String,
    pub label: String,
    pub explanation: String,
    pub recommended: bool,
    pub documentation: Vec<BlobshDocRef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobshContextHelp {
    pub subject: String,
    pub explanation: String,
    pub documentation: Vec<BlobshDocRef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlobshTraceError {
    EmptyText,
    MissingDepth(BlobshDepth),
    LayerNotEditable(BlobshDepth),
    DuplicateDepth(BlobshDepth),
    NonMonotonicDepth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobshCommandTrace {
    layers: Vec<BlobshCommandLayer>,
    active_depth: BlobshDepth,
    status: BlobshCommandStatus,
    assistance: BlobshAssistanceMode,
}

impl BlobshCommandTrace {
    pub fn from_user_input(text: impl Into<String>) -> Result<Self, BlobshTraceError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(BlobshTraceError::EmptyText);
        }

        Ok(Self {
            layers: vec![BlobshCommandLayer::new(
                BlobshDepth::User,
                text,
                true,
                BlobshProvenance::UserInput,
            )],
            active_depth: BlobshDepth::User,
            status: BlobshCommandStatus::Preview,
            assistance: BlobshAssistanceMode::Assist,
        })
    }

    pub fn layers(&self) -> &[BlobshCommandLayer] {
        &self.layers
    }

    pub const fn active_depth(&self) -> BlobshDepth {
        self.active_depth
    }

    pub const fn status(&self) -> BlobshCommandStatus {
        self.status
    }

    pub const fn assistance(&self) -> BlobshAssistanceMode {
        self.assistance
    }

    pub fn set_assistance(&mut self, assistance: BlobshAssistanceMode) {
        self.assistance = assistance;
    }

    pub fn set_status(&mut self, status: BlobshCommandStatus) {
        self.status = status;
    }

    pub fn append_layer(&mut self, layer: BlobshCommandLayer) -> Result<(), BlobshTraceError> {
        if layer.text.trim().is_empty() {
            return Err(BlobshTraceError::EmptyText);
        }
        if self.layers.iter().any(|existing| existing.depth == layer.depth) {
            return Err(BlobshTraceError::DuplicateDepth(layer.depth));
        }
        let expected = self
            .layers
            .last()
            .and_then(|last| last.depth.deeper())
            .ok_or(BlobshTraceError::NonMonotonicDepth)?;
        if layer.depth != expected {
            return Err(BlobshTraceError::NonMonotonicDepth);
        }
        self.active_depth = layer.depth;
        self.layers.push(layer);
        Ok(())
    }

    pub fn layer(&self, depth: BlobshDepth) -> Option<&BlobshCommandLayer> {
        self.layers.iter().find(|layer| layer.depth == depth)
    }

    pub fn select_depth(&mut self, depth: BlobshDepth) -> Result<(), BlobshTraceError> {
        if self.layer(depth).is_none() {
            return Err(BlobshTraceError::MissingDepth(depth));
        }
        self.active_depth = depth;
        Ok(())
    }

    pub fn active_layer(&self) -> &BlobshCommandLayer {
        self.layer(self.active_depth)
            .expect("active depth always refers to an existing layer")
    }

    /// Replace one visible representation and invalidate every deeper derived
    /// representation. Editing NATIVE is recorded as a native user override;
    /// editing any higher layer requests deterministic regeneration below it.
    pub fn edit_layer(
        &mut self,
        depth: BlobshDepth,
        replacement: impl Into<String>,
    ) -> Result<(), BlobshTraceError> {
        let replacement = replacement.into();
        if replacement.trim().is_empty() {
            return Err(BlobshTraceError::EmptyText);
        }

        let index = self
            .layers
            .iter()
            .position(|layer| layer.depth == depth)
            .ok_or(BlobshTraceError::MissingDepth(depth))?;
        if !self.layers[index].editable {
            return Err(BlobshTraceError::LayerNotEditable(depth));
        }

        self.layers[index].text = replacement;
        self.layers[index].provenance = if depth == BlobshDepth::Native {
            BlobshProvenance::UserNativeOverride
        } else {
            BlobshProvenance::UserEdited
        };
        self.layers.truncate(index + 1);
        self.active_depth = depth;
        self.status = if depth == BlobshDepth::Native {
            BlobshCommandStatus::Preview
        } else {
            BlobshCommandStatus::NeedsRegeneration
        };
        Ok(())
    }

    pub fn depth_indicator(&self) -> String {
        format!(
            "{}/{} {}",
            self.active_depth.index() + 1,
            BlobshDepth::ALL.len(),
            self.active_depth.as_str()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_trace() -> BlobshCommandTrace {
        let mut trace = BlobshCommandTrace::from_user_input("attiva bluetooth").unwrap();
        for (depth, text, provenance) in [
            (
                BlobshDepth::Intent,
                "system.bluetooth enable",
                BlobshProvenance::AiProposed,
            ),
            (
                BlobshDepth::Plan,
                "capability=bluetooth.manage target=local",
                BlobshProvenance::Derived,
            ),
            (
                BlobshDepth::Spec,
                "feature.bluetooth = enabled",
                BlobshProvenance::Derived,
            ),
            (
                BlobshDepth::Backend,
                "hardware.bluetooth.enable = true",
                BlobshProvenance::BackendGenerated,
            ),
            (
                BlobshDepth::Native,
                "nixos-rebuild switch --flake .#local",
                BlobshProvenance::BackendGenerated,
            ),
        ] {
            trace
                .append_layer(BlobshCommandLayer::new(depth, text, true, provenance))
                .unwrap();
        }
        trace
    }

    #[test]
    fn depth_navigation_exposes_the_full_user_to_native_chain() {
        let trace = complete_trace();
        assert_eq!(trace.layers().len(), BlobshDepth::ALL.len());
        assert_eq!(trace.active_depth(), BlobshDepth::Native);
        assert_eq!(trace.depth_indicator(), "6/6 NATIVE");
    }

    #[test]
    fn editing_intent_invalidates_every_deeper_representation() {
        let mut trace = complete_trace();
        trace
            .edit_layer(BlobshDepth::Intent, "system.bluetooth diagnose")
            .unwrap();

        assert_eq!(trace.layers().len(), 2);
        assert_eq!(trace.active_depth(), BlobshDepth::Intent);
        assert_eq!(trace.status(), BlobshCommandStatus::NeedsRegeneration);
        assert_eq!(
            trace.active_layer().provenance,
            BlobshProvenance::UserEdited
        );
    }

    #[test]
    fn editing_native_is_preserved_as_an_explicit_user_override() {
        let mut trace = complete_trace();
        trace
            .edit_layer(
                BlobshDepth::Native,
                "nixos-rebuild test --flake .#local",
            )
            .unwrap();

        assert_eq!(trace.layers().len(), 6);
        assert_eq!(
            trace.active_layer().provenance,
            BlobshProvenance::UserNativeOverride
        );
        assert_eq!(trace.status(), BlobshCommandStatus::Preview);
    }

    #[test]
    fn layers_must_be_added_in_monotonic_depth_order() {
        let mut trace = BlobshCommandTrace::from_user_input("hello").unwrap();
        assert_eq!(
            trace.append_layer(BlobshCommandLayer::new(
                BlobshDepth::Plan,
                "skip intent",
                true,
                BlobshProvenance::Derived,
            )),
            Err(BlobshTraceError::NonMonotonicDepth)
        );
    }
}
