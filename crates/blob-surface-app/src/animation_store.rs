use crate::character::{
    BlobAction, BlobAnimationPackError, BlobAnimationPackManifest, BlobRendererKind,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobPackTrust {
    BuiltIn,
    SignedCommunity,
    LocalUnverified,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledBlobAnimationPack {
    pub manifest: BlobAnimationPackManifest,
    pub trust: BlobPackTrust,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlobAnimationStoreError {
    InvalidPack(BlobAnimationPackError),
    PackAlreadyInstalled(String),
    PackNotInstalled(String),
    RendererMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobAnimationRegistry {
    installed: Vec<InstalledBlobAnimationPack>,
    active_soft3d: Option<String>,
    active_pixel: Option<String>,
    active_ascii: Option<String>,
}

impl Default for BlobAnimationRegistry {
    fn default() -> Self {
        Self {
            installed: Vec::new(),
            active_soft3d: None,
            active_pixel: None,
            active_ascii: None,
        }
    }
}

impl BlobAnimationRegistry {
    pub fn install(
        &mut self,
        pack: InstalledBlobAnimationPack,
    ) -> Result<(), BlobAnimationStoreError> {
        pack.manifest
            .validate()
            .map_err(BlobAnimationStoreError::InvalidPack)?;

        if self
            .installed
            .iter()
            .any(|installed| installed.manifest.id == pack.manifest.id)
        {
            return Err(BlobAnimationStoreError::PackAlreadyInstalled(
                pack.manifest.id.clone(),
            ));
        }

        self.installed.push(pack);
        Ok(())
    }

    pub fn installed(&self) -> &[InstalledBlobAnimationPack] {
        &self.installed
    }

    pub fn activate(
        &mut self,
        renderer: BlobRendererKind,
        pack_id: &str,
    ) -> Result<(), BlobAnimationStoreError> {
        let pack = self
            .installed
            .iter()
            .find(|pack| pack.manifest.id == pack_id)
            .ok_or_else(|| BlobAnimationStoreError::PackNotInstalled(pack_id.to_owned()))?;

        if pack.manifest.renderer != renderer {
            return Err(BlobAnimationStoreError::RendererMismatch);
        }

        let slot = match renderer {
            BlobRendererKind::Soft3d => &mut self.active_soft3d,
            BlobRendererKind::Pixel => &mut self.active_pixel,
            BlobRendererKind::Ascii => &mut self.active_ascii,
        };
        *slot = Some(pack_id.to_owned());
        Ok(())
    }

    pub fn active_pack(&self, renderer: BlobRendererKind) -> Option<&InstalledBlobAnimationPack> {
        let id = match renderer {
            BlobRendererKind::Soft3d => self.active_soft3d.as_deref(),
            BlobRendererKind::Pixel => self.active_pixel.as_deref(),
            BlobRendererKind::Ascii => self.active_ascii.as_deref(),
        }?;

        self.installed
            .iter()
            .find(|pack| pack.manifest.id == id)
    }

    pub fn resolve_action(&self, renderer: BlobRendererKind, requested: BlobAction) -> BlobAction {
        self.active_pack(renderer)
            .map(|pack| pack.manifest.resolved_action(requested))
            .unwrap_or(BlobAction::Idle)
    }
}

#[cfg(test)]
mod tests {
    use crate::character::{BlobAnimationClipSpec, BlobAnimationPackManifest};

    use super::*;

    fn pack(id: &str, renderer: BlobRendererKind, actions: &[BlobAction]) -> InstalledBlobAnimationPack {
        InstalledBlobAnimationPack {
            manifest: BlobAnimationPackManifest {
                schema_version: BlobAnimationPackManifest::CURRENT_SCHEMA_VERSION,
                id: id.into(),
                version: "0.1.0".into(),
                renderer,
                creator: "test".into(),
                license: "CC-BY-4.0".into(),
                character_family: "blob-gremlin-v1".into(),
                clips: actions
                    .iter()
                    .copied()
                    .map(|action| BlobAnimationClipSpec {
                        action,
                        frame_count: 1,
                        frames_per_second: 1,
                        loops: action == BlobAction::Idle,
                        interruptible: true,
                    })
                    .collect(),
                content_digest: format!("sha256:{id}"),
                ai_generated: true,
                generation_provenance: Some("test fixture".into()),
            },
            trust: BlobPackTrust::LocalUnverified,
            source: "local:test".into(),
        }
    }

    #[test]
    fn store_install_requires_a_valid_data_only_manifest() {
        let mut registry = BlobAnimationRegistry::default();
        let mut invalid = pack("bad", BlobRendererKind::Soft3d, &[BlobAction::Wave]);
        assert_eq!(
            registry.install(invalid.clone()),
            Err(BlobAnimationStoreError::InvalidPack(
                BlobAnimationPackError::MissingIdleClip
            ))
        );

        invalid.manifest.clips.push(BlobAnimationClipSpec {
            action: BlobAction::Idle,
            frame_count: 1,
            frames_per_second: 1,
            loops: true,
            interruptible: true,
        });
        assert_eq!(registry.install(invalid), Ok(()));
    }

    #[test]
    fn active_pack_is_scoped_to_its_renderer() {
        let mut registry = BlobAnimationRegistry::default();
        registry
            .install(pack(
                "soft",
                BlobRendererKind::Soft3d,
                &[BlobAction::Idle, BlobAction::Wave],
            ))
            .unwrap();
        registry
            .install(pack("ascii", BlobRendererKind::Ascii, &[BlobAction::Idle]))
            .unwrap();

        registry
            .activate(BlobRendererKind::Soft3d, "soft")
            .unwrap();
        assert_eq!(
            registry.activate(BlobRendererKind::Ascii, "soft"),
            Err(BlobAnimationStoreError::RendererMismatch)
        );
        assert_eq!(
            registry.resolve_action(BlobRendererKind::Soft3d, BlobAction::Wave),
            BlobAction::Wave
        );
        assert_eq!(
            registry.resolve_action(BlobRendererKind::Soft3d, BlobAction::Sleep),
            BlobAction::Idle
        );
    }

    #[test]
    fn duplicate_pack_ids_are_rejected() {
        let mut registry = BlobAnimationRegistry::default();
        registry
            .install(pack("same", BlobRendererKind::Soft3d, &[BlobAction::Idle]))
            .unwrap();
        assert_eq!(
            registry.install(pack("same", BlobRendererKind::Soft3d, &[BlobAction::Idle])),
            Err(BlobAnimationStoreError::PackAlreadyInstalled("same".into()))
        );
    }
}
