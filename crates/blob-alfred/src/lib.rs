#![forbid(unsafe_code)]

use std::collections::HashSet;

use blob_core::{Event, EventId, Situation, SituationId, SituationWindow, SubjectRef, WorkspaceId};

pub trait CorrelationRule: Send + Sync {
    fn name(&self) -> &str;
    fn correlate(&self, event: &Event) -> Option<Situation>;
}

#[derive(Debug, PartialEq, Eq)]
pub struct IngestOutcome {
    pub duplicate: bool,
    pub situations: Vec<Situation>,
}

pub struct Alfred {
    seen_event_ids: HashSet<EventId>,
    rules: Vec<Box<dyn CorrelationRule>>,
}

impl Alfred {
    pub fn new(rules: Vec<Box<dyn CorrelationRule>>) -> Self {
        Self {
            seen_event_ids: HashSet::new(),
            rules,
        }
    }

    pub fn ingest(&mut self, event: Event) -> IngestOutcome {
        if !self.seen_event_ids.insert(event.id.clone()) {
            return IngestOutcome {
                duplicate: true,
                situations: Vec::new(),
            };
        }

        let situations = self
            .rules
            .iter()
            .filter_map(|rule| rule.correlate(&event))
            .collect();

        IngestOutcome {
            duplicate: false,
            situations,
        }
    }

    pub fn seen_event_count(&self) -> usize {
        self.seen_event_ids.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceChangeRequiresTestRule {
    pub active_workspace: WorkspaceId,
    pub rule_version: String,
}

impl SourceChangeRequiresTestRule {
    pub fn v1(active_workspace: WorkspaceId) -> Self {
        Self {
            active_workspace,
            rule_version: "development.source-change-requires-test@v1".into(),
        }
    }
}

impl CorrelationRule for SourceChangeRequiresTestRule {
    fn name(&self) -> &str {
        &self.rule_version
    }

    fn correlate(&self, event: &Event) -> Option<Situation> {
        if event.kind != "source.modified" {
            return None;
        }

        let belongs_to_active_workspace = event.subjects.iter().any(|subject| {
            matches!(subject, SubjectRef::Workspace(id) if id == &self.active_workspace)
        });

        if !belongs_to_active_workspace {
            return None;
        }

        Some(Situation {
            id: SituationId::new(format!(
                "situation:{}:{}",
                self.rule_version,
                event.id.as_str()
            )),
            kind: "development.source-change-requires-test".into(),
            summary: "Relevant source changed; the active Development Workspace should reconsider tests."
                .into(),
            evidence_event_ids: vec![event.id.clone()],
            derived_facts: vec![("test_required".into(), "true".into())],
            subjects: event.subjects.clone(),
            window: SituationWindow {
                start_unix_ms: event.observed_at_unix_ms,
                end_unix_ms: event.observed_at_unix_ms,
            },
            confidence_ppm: None,
            deterministic_rule_provenance: vec![self.rule_version.clone()],
            semantic_provenance: Vec::new(),
            expires_at_unix_ms: None,
        })
    }
}
