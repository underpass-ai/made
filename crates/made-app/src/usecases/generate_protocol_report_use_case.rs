use std::fmt;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::ProtocolEventStorePort;
use made_core::value_objects::ProtocolId;

use super::{
    GenerateProtocolReportInput, ProtocolInstanceView, ProtocolReport, ProtocolReportBinding,
    ReadWholeProtocolEventsUseCase, ResolveProtocolDefinitionUseCase,
};
use crate::services::SessionStream;

mod protocol_report_markdown;
#[cfg(test)]
mod report_cut_store;

use protocol_report_markdown::{render_markdown, ReportedSession};

/// Renders a report over one or more persisted sessions.
///
/// ADR-006: a report is a projection of persisted state, never a
/// document the engine stores. Nothing here writes, and asking twice
/// for the same sessions in the same state answers the same bytes.
///
/// It composes three reads — the session, the definition it runs and
/// its event stream — because a report that quoted the session without
/// the stream would be a summary of an audit rather than the audit.
/// Composing them here rather than in an adapter is what lets both
/// editions answer with the same document (parity slice F3c).
pub struct GenerateProtocolReportUseCase {
    definitions: Arc<ResolveProtocolDefinitionUseCase>,
    events: Arc<dyn ProtocolEventStorePort>,
}

impl fmt::Debug for GenerateProtocolReportUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerateProtocolReportUseCase")
            .finish()
    }
}

impl GenerateProtocolReportUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveProtocolDefinitionUseCase>,
        events: Arc<dyn ProtocolEventStorePort>,
    ) -> Self {
        Self {
            definitions,
            events,
        }
    }

    #[tracing::instrument(name = "generate_protocol_report", skip_all)]
    pub async fn execute(
        &self,
        input: GenerateProtocolReportInput,
    ) -> Result<ProtocolReport, DomainError> {
        let mut sessions = Vec::with_capacity(input.protocol_ids().len());
        for protocol_id in input.protocol_ids() {
            sessions.push(self.load(protocol_id).await?);
        }

        let bindings = sessions
            .iter()
            .map(|session| {
                ProtocolReportBinding::new(
                    session.instance.id().clone(),
                    session.definition.name().clone(),
                    session.definition.version().clone(),
                    session.digest,
                    session.instance.bound_definition(),
                    session.completed,
                )
            })
            .collect();

        Ok(ProtocolReport::new(
            render_markdown(input.title(), &sessions)?,
            bindings,
        ))
    }

    /// One session, with everything the report quotes of it.
    async fn load(&self, protocol_id: &ProtocolId) -> Result<ReportedSession, DomainError> {
        // One bounded stream read defines the report cut. Both the fold and
        // audit journal below use these exact records, so an append racing the
        // report can only fall wholly before or wholly after this projection.
        let journal = ReadWholeProtocolEventsUseCase::new(self.events.clone())
            .execute(protocol_id)
            .await?;
        let instance = SessionStream::fold_records(&journal)?.instance;
        // The pin's digest is the publication's own for a bound session,
        // so the report never shows a recomputed digest beside the one
        // the session recorded.
        let (definition, pin) = self.definitions.execute_pinned(&instance).await?;
        let completed = ProtocolInstanceView::project(&instance, &definition)?.is_completed();
        let digest = pin.digest();
        Ok(ReportedSession {
            definition,
            instance,
            journal,
            completed,
            digest,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use made_core::entities::ProtocolInstance;
    use made_core::entities::{AuditFact, ProtocolEvent};
    use made_core::ports::{ProtocolEventStorePort, ProtocolSnapshotStorePort};
    use made_core::value_objects::{
        Attributes, AuditActorKind, EventId, ProtocolContext, StepOutput, StepResult, StreamVersion,
    };

    use super::*;
    use crate::usecases::protocol_test_support::{
        definition, definition_resolver, lease_owner, lease_ttl, now, protocol_id,
        started_instance, stream, DefinitionRepositoryFake, EventStoreFake, FixedClock,
        StepHandlerFake,
    };
    use crate::usecases::{ReportTitle, RunProtocolInput, RunProtocolUseCase};
    use report_cut_store::ReportCutStore;

    struct Fixture {
        usecase: GenerateProtocolReportUseCase,
    }

    async fn fixture() -> Fixture {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let store = Arc::new(EventStoreFake::default());
        store.save(&started_instance(&definition)).await.unwrap();
        // A second session of the same definition, so a report over
        // two ids has something to keep in order.
        let other = ProtocolInstance::start(
            ProtocolId::new("session-2").unwrap(),
            &definition,
            ProtocolContext::empty(),
            now(),
        )
        .expect("required protocol inputs");
        store.save(&other).await.unwrap();

        Fixture {
            usecase: GenerateProtocolReportUseCase::new(definition_resolver(definitions), store),
        }
    }

    fn ids(values: &[&str]) -> Vec<ProtocolId> {
        values
            .iter()
            .map(|value| ProtocolId::new(*value).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn a_report_projects_the_session_its_definition_and_its_stream() {
        let fixture = fixture().await;

        let report = fixture
            .usecase
            .execute(
                GenerateProtocolReportInput::new(
                    vec![protocol_id()],
                    Some(ReportTitle::new("Session review").unwrap()),
                )
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(report.protocol_count(), 1);
        assert_eq!(report.completed_count(), 0);
        assert_eq!(report.incomplete_count(), 1);
        assert!(report.markdown().starts_with("# Session review\n\n"));
        for section in [
            "### Definition",
            "### Steps and outputs",
            "### Transitions",
            "### Guard approvals",
            "### Guard deferrals",
            "### Interventions and evidence",
            "### Reasons",
            "### Audit journal",
        ] {
            assert!(
                report.markdown().contains(section),
                "missing {section}: {}",
                report.markdown()
            );
        }
        // The stream is in there, not a summary of it.
        assert!(report.markdown().contains("protocol_instance_started"));

        let binding = &report.bindings()[0];
        assert_eq!(binding.protocol_id(), &protocol_id());
        assert_eq!(binding.definition_name(), definition().name());
        assert!(!binding.completed());
        // Started from a document, not from a published version.
        assert!(binding.bound_definition_digest().is_none());
    }

    /// A session bound to a publication sealed under the Choreographer
    /// scheme reports that publication's digest, not one recomputed under
    /// the current scheme: one digest, the one the session recorded.
    #[tokio::test]
    async fn a_session_bound_to_a_choreographer_publication_reports_its_recorded_digest() {
        use made_core::entities::PublishedProtocolDefinition;
        use made_core::value_objects::ProtocolDefinitionDigestScheme;

        use crate::usecases::protocol_test_support::{resolver_with, PublicationsFake};

        let definition = definition();
        let legacy_digest = definition
            .digest_under(ProtocolDefinitionDigestScheme::ChoreographerV1)
            .unwrap();
        let publications = Arc::new(PublicationsFake::default());
        let published = publications
            .seed_published(
                PublishedProtocolDefinition::verify(definition.clone(), legacy_digest).unwrap(),
            )
            .await;
        let store = Arc::new(EventStoreFake::default());
        store
            .save(
                &ProtocolInstance::start_bound(
                    protocol_id(),
                    &published,
                    ProtocolContext::empty(),
                    now(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let usecase = GenerateProtocolReportUseCase::new(
            resolver_with(
                Arc::new(DefinitionRepositoryFake::new(definition.clone())),
                publications,
            ),
            store,
        );

        let report = usecase
            .execute(GenerateProtocolReportInput::new(vec![protocol_id()], None).unwrap())
            .await
            .unwrap();

        let binding = &report.bindings()[0];
        assert_eq!(binding.definition_digest(), &legacy_digest);
        assert_eq!(binding.bound_definition_digest(), Some(&legacy_digest));
        assert_ne!(legacy_digest, definition.digest().unwrap());
        assert!(!report
            .markdown()
            .contains(&definition.digest().unwrap().to_hex()));
    }

    /// Read-only means asking twice answers the same bytes. A report
    /// that drifted between two reads of one unchanged session would
    /// not be a projection of it.
    #[tokio::test]
    async fn the_same_state_reports_the_same_bytes() {
        let fixture = fixture().await;
        let input = GenerateProtocolReportInput::new(ids(&["session-2"]), None).unwrap();

        let first = fixture.usecase.execute(input.clone()).await.unwrap();
        let second = fixture.usecase.execute(input).await.unwrap();

        assert_eq!(first, second);
        assert!(first.markdown().starts_with("# Protocol report\n\n"));
    }

    #[tokio::test]
    async fn an_append_during_generation_is_wholly_after_the_report_cut() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let inner = Arc::new(EventStoreFake::default());
        let instance = started_instance(&definition);
        inner.save(&instance).await.unwrap();
        let store = Arc::new(ReportCutStore::new(inner.clone()));
        let usecase = Arc::new(GenerateProtocolReportUseCase::new(
            definition_resolver(definitions),
            store.clone(),
        ));

        let report = tokio::spawn({
            let usecase = usecase.clone();
            async move {
                usecase
                    .execute(GenerateProtocolReportInput::new(vec![protocol_id()], None).unwrap())
                    .await
            }
        });
        store.wait_until_captured().await;

        let head = inner.records(&protocol_id()).await.pop().unwrap();
        let completion = AuditFact {
            event_id: EventId::new("concurrent-completion").unwrap(),
            event: ProtocolEvent::ProtocolCompleted(
                made_core::entities::protocol_events::ProtocolCompleted {
                    final_state: instance.current_state().clone(),
                    completed_at: now(),
                },
            ),
            protocol_id: protocol_id(),
            definition_name: definition.name().clone(),
            definition_version: definition.version().clone(),
            occurred_at: now(),
            actor: made_core::value_objects::AuditActor::new(
                "concurrent-writer",
                AuditActorKind::Service,
                None,
            )
            .unwrap(),
            correlation_id: head.correlation_id().cloned(),
            causation_id: Some(head.event_id().clone()),
            trace: None,
        };
        assert!(inner
            .append(&protocol_id(), StreamVersion::new(1), vec![completion])
            .await
            .unwrap()
            .appended_version()
            .is_some());
        store.release();

        let report = report.await.unwrap().unwrap();
        assert_eq!(store.read_count(), 1);
        assert_eq!(report.incomplete_count(), 1);
        assert!(!report.markdown().contains("concurrent-completion"));
        assert!(!report.markdown().contains("protocol_completed"));
    }

    #[tokio::test]
    async fn the_report_keeps_the_order_the_caller_asked_for() {
        let fixture = fixture().await;

        let report = fixture
            .usecase
            .execute(
                GenerateProtocolReportInput::new(ids(&["session-2", "protocol-1"]), None).unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            report
                .bindings()
                .iter()
                .map(|binding| binding.protocol_id().as_str().to_owned())
                .collect::<Vec<_>>(),
            ["session-2", "protocol-1"]
        );
    }

    /// The claim slice A5 makes: the report renders from the stream
    /// and the definition, and from nothing else.
    ///
    /// Snapshots are a cache (ADR-012), so every one of them is thrown
    /// away before the report is asked for. What is left is the
    /// records, and the step output a reader finds in the document is
    /// the one the `StepCompleted` record carries.
    #[tokio::test]
    async fn a_report_renders_step_outputs_from_the_records_alone() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let store = Arc::new(EventStoreFake::default());
        let output = StepOutput::new(
            Attributes::new(BTreeMap::from([(
                "winner_content".to_owned(),
                serde_json::json!("ship the smaller scope first"),
            )]))
            .unwrap(),
        );
        RunProtocolUseCase::new(
            definitions.clone(),
            stream(store.clone()),
            Arc::new(StepHandlerFake::succeeding(
                StepResult::completed(output).unwrap(),
            )),
            Arc::new(FixedClock::new(now())),
        )
        .execute(RunProtocolInput::new(
            protocol_id(),
            definition,
            ProtocolContext::empty(),
            lease_owner(),
            lease_ttl(),
            "operator-1",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();

        // Every cached fold, gone. The stream is the whole of what is
        // left, and the report is asked the same question.
        store.forget(&protocol_id()).await.unwrap();
        let usecase = GenerateProtocolReportUseCase::new(definition_resolver(definitions), store);

        let report = usecase
            .execute(GenerateProtocolReportInput::new(vec![protocol_id()], None).unwrap())
            .await
            .unwrap();

        assert_eq!(report.completed_count(), 1);
        assert!(
            report.markdown().contains("ship the smaller scope first"),
            "the step output is not in the report: {}",
            report.markdown()
        );
        assert!(report.markdown().contains("step_completed"));
    }

    #[tokio::test]
    async fn a_session_that_is_not_there_is_not_found() {
        let fixture = fixture().await;

        let error = fixture
            .usecase
            .execute(GenerateProtocolReportInput::new(ids(&["missing"]), None).unwrap())
            .await
            .unwrap_err();

        assert!(matches!(error, DomainError::NotFound { .. }));
    }
}
