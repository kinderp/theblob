#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimarySurface {
    Now,
    CurrentWorkspace,
    History,
    Fabric,
    Inspector,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TechnicianIntent {
    ExplainCurrent,
    TeachCurrent,
    PrepareNextStep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceIntent {
    Navigate(PrimarySurface),
    OpenInspector,
    CloseInspector,
    Technician(TechnicianIntent),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceEffect {
    PageChanged(PrimarySurface),
    TechnicianIntentRecorded(TechnicianIntent),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceApplication {
    current: PrimarySurface,
    inspector_return: PrimarySurface,
    last_technician_intent: Option<TechnicianIntent>,
}

impl Default for SurfaceApplication {
    fn default() -> Self {
        Self {
            current: PrimarySurface::Now,
            inspector_return: PrimarySurface::Now,
            last_technician_intent: None,
        }
    }
}

impl SurfaceApplication {
    pub fn current(&self) -> PrimarySurface {
        self.current
    }

    pub fn last_technician_intent(&self) -> Option<TechnicianIntent> {
        self.last_technician_intent
    }

    /// Apply one renderer-neutral user intent.
    ///
    /// This layer deliberately has no executor, D-Bus, package-manager or NixOS
    /// dependency. Recording a Technician intent is not execution authority.
    pub fn apply(&mut self, intent: SurfaceIntent) -> SurfaceEffect {
        match intent {
            SurfaceIntent::Navigate(page) => {
                self.current = page;
                SurfaceEffect::PageChanged(page)
            }
            SurfaceIntent::OpenInspector => {
                if self.current != PrimarySurface::Inspector {
                    self.inspector_return = self.current;
                }
                self.current = PrimarySurface::Inspector;
                SurfaceEffect::PageChanged(PrimarySurface::Inspector)
            }
            SurfaceIntent::CloseInspector => {
                self.current = self.inspector_return;
                SurfaceEffect::PageChanged(self.current)
            }
            SurfaceIntent::Technician(request) => {
                self.last_technician_intent = Some(request);
                SurfaceEffect::TechnicianIntentRecorded(request)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspector_returns_to_the_surface_that_opened_it() {
        let mut app = SurfaceApplication::default();
        app.apply(SurfaceIntent::Navigate(PrimarySurface::CurrentWorkspace));
        assert_eq!(
            app.apply(SurfaceIntent::OpenInspector),
            SurfaceEffect::PageChanged(PrimarySurface::Inspector)
        );
        assert_eq!(
            app.apply(SurfaceIntent::CloseInspector),
            SurfaceEffect::PageChanged(PrimarySurface::CurrentWorkspace)
        );
    }

    #[test]
    fn technician_prepare_records_intent_without_navigating_or_executing() {
        let mut app = SurfaceApplication::default();
        let before = app.current();
        assert_eq!(
            app.apply(SurfaceIntent::Technician(TechnicianIntent::PrepareNextStep)),
            SurfaceEffect::TechnicianIntentRecorded(TechnicianIntent::PrepareNextStep)
        );
        assert_eq!(app.current(), before);
        assert_eq!(
            app.last_technician_intent(),
            Some(TechnicianIntent::PrepareNextStep)
        );
    }

    #[test]
    fn navigation_is_explicit_and_renderer_independent() {
        let mut app = SurfaceApplication::default();
        for page in [
            PrimarySurface::History,
            PrimarySurface::Fabric,
            PrimarySurface::Now,
        ] {
            assert_eq!(
                app.apply(SurfaceIntent::Navigate(page)),
                SurfaceEffect::PageChanged(page)
            );
            assert_eq!(app.current(), page);
        }
    }
}
