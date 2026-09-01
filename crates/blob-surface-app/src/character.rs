#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlobAction {
    Idle,
    Blink,
    Look,
    Grin,
    Wave,
    Busy,
    Warning,
    Sleep,
}

impl BlobAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Blink => "blink",
            Self::Look => "look",
            Self::Grin => "grin",
            Self::Wave => "wave",
            Self::Busy => "busy",
            Self::Warning => "warning",
            Self::Sleep => "sleep",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlobBehaviorContext {
    pub hovered: bool,
    pub selected: bool,
    pub task_running: bool,
    pub problem: bool,
    pub long_idle: bool,
}

impl BlobBehaviorContext {
    pub const fn calm() -> Self {
        Self {
            hovered: false,
            selected: false,
            task_running: false,
            problem: false,
            long_idle: false,
        }
    }

    /// Resolve semantic character state without renderer knowledge.
    ///
    /// Higher-priority real system/workspace conditions win over decorative
    /// interaction states. A renderer may still add non-semantic micro-motion
    /// such as an occasional blink while `Idle` is active.
    pub const fn resolve(self) -> BlobAction {
        if self.problem {
            BlobAction::Warning
        } else if self.task_running {
            BlobAction::Busy
        } else if self.selected {
            BlobAction::Wave
        } else if self.hovered {
            BlobAction::Look
        } else if self.long_idle {
            BlobAction::Sleep
        } else {
            BlobAction::Idle
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobRendererKind {
    Soft3d,
    Pixel,
    Ascii,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobAnimationClipSpec {
    pub action: BlobAction,
    pub frame_count: u16,
    pub frames_per_second: u16,
    pub loops: bool,
    pub interruptible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobAnimationPackManifest {
    pub schema_version: u32,
    pub id: String,
    pub version: String,
    pub renderer: BlobRendererKind,
    pub creator: String,
    pub license: String,
    pub character_family: String,
    pub clips: Vec<BlobAnimationClipSpec>,
    pub content_digest: String,
    pub ai_generated: bool,
    pub generation_provenance: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlobAnimationPackError {
    UnsupportedSchemaVersion,
    MissingPackId,
    MissingVersion,
    MissingCreator,
    MissingLicense,
    MissingCharacterFamily,
    MissingContentDigest,
    MissingIdleClip,
    DuplicateAction(BlobAction),
    ZeroFrameClip(BlobAction),
    InvalidFrameRate(BlobAction),
}

impl BlobAnimationPackManifest {
    pub const CURRENT_SCHEMA_VERSION: u32 = 1;

    /// Validate a pack before a renderer or future store accepts it.
    ///
    /// Animation packs are deliberately data-only. They describe media for
    /// already-known semantic actions; they do not contain executable hooks or
    /// system authority.
    pub fn validate(&self) -> Result<(), BlobAnimationPackError> {
        if self.schema_version != Self::CURRENT_SCHEMA_VERSION {
            return Err(BlobAnimationPackError::UnsupportedSchemaVersion);
        }
        if self.id.trim().is_empty() {
            return Err(BlobAnimationPackError::MissingPackId);
        }
        if self.version.trim().is_empty() {
            return Err(BlobAnimationPackError::MissingVersion);
        }
        if self.creator.trim().is_empty() {
            return Err(BlobAnimationPackError::MissingCreator);
        }
        if self.license.trim().is_empty() {
            return Err(BlobAnimationPackError::MissingLicense);
        }
        if self.character_family.trim().is_empty() {
            return Err(BlobAnimationPackError::MissingCharacterFamily);
        }
        if self.content_digest.trim().is_empty() {
            return Err(BlobAnimationPackError::MissingContentDigest);
        }

        let mut seen = Vec::with_capacity(self.clips.len());
        for clip in &self.clips {
            if clip.frame_count == 0 {
                return Err(BlobAnimationPackError::ZeroFrameClip(clip.action));
            }
            if clip.frames_per_second == 0 || clip.frames_per_second > 30 {
                return Err(BlobAnimationPackError::InvalidFrameRate(clip.action));
            }
            if seen.contains(&clip.action) {
                return Err(BlobAnimationPackError::DuplicateAction(clip.action));
            }
            seen.push(clip.action);
        }

        if !seen.contains(&BlobAction::Idle) {
            return Err(BlobAnimationPackError::MissingIdleClip);
        }

        Ok(())
    }

    /// Missing optional actions safely fall back to the pack's idle clip.
    pub fn resolved_action(&self, requested: BlobAction) -> BlobAction {
        if self.clips.iter().any(|clip| clip.action == requested) {
            requested
        } else {
            BlobAction::Idle
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_pack() -> BlobAnimationPackManifest {
        BlobAnimationPackManifest {
            schema_version: BlobAnimationPackManifest::CURRENT_SCHEMA_VERSION,
            id: "community.gremlins.calm".into(),
            version: "0.1.0".into(),
            renderer: BlobRendererKind::Soft3d,
            creator: "The Blob community".into(),
            license: "CC-BY-4.0".into(),
            character_family: "blob-gremlin-v1".into(),
            clips: vec![
                BlobAnimationClipSpec {
                    action: BlobAction::Idle,
                    frame_count: 2,
                    frames_per_second: 2,
                    loops: true,
                    interruptible: true,
                },
                BlobAnimationClipSpec {
                    action: BlobAction::Wave,
                    frame_count: 4,
                    frames_per_second: 6,
                    loops: false,
                    interruptible: true,
                },
            ],
            content_digest: "sha256:test".into(),
            ai_generated: true,
            generation_provenance: Some("user-approved AI generation".into()),
        }
    }

    #[test]
    fn real_workspace_conditions_beat_decorative_interaction() {
        let context = BlobBehaviorContext {
            hovered: true,
            selected: true,
            task_running: true,
            problem: true,
            long_idle: true,
        };
        assert_eq!(context.resolve(), BlobAction::Warning);
    }

    #[test]
    fn calm_context_resolves_to_idle() {
        assert_eq!(BlobBehaviorContext::calm().resolve(), BlobAction::Idle);
    }

    #[test]
    fn pack_requires_idle_and_rejects_duplicate_actions() {
        let mut pack = valid_pack();
        assert_eq!(pack.validate(), Ok(()));

        pack.clips.push(BlobAnimationClipSpec {
            action: BlobAction::Wave,
            frame_count: 1,
            frames_per_second: 1,
            loops: false,
            interruptible: true,
        });
        assert_eq!(
            pack.validate(),
            Err(BlobAnimationPackError::DuplicateAction(BlobAction::Wave))
        );
    }

    #[test]
    fn missing_optional_clip_falls_back_to_idle() {
        let pack = valid_pack();
        assert_eq!(pack.resolved_action(BlobAction::Sleep), BlobAction::Idle);
        assert_eq!(pack.resolved_action(BlobAction::Wave), BlobAction::Wave);
    }
}
