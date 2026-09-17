use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{CeremonyId, StreamVersion};

use super::{
    CeremonyInstanceView, CeremonyReport, CeremonyReportBinding, GenerateCeremonyReportInput,
    GetCeremonyInstanceUseCase, ResolveCeremonyDefinitionUseCase,
};

mod ceremony_report_markdown;

use ceremony_report_markdown::{render_markdown, ReportedSession};

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
pub struct GenerateCeremonyReportUseCase {
    instances: Arc<GetCeremonyInstanceUseCase>,
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    events: Arc<dyn CeremonyEventStorePort>,
}

impl fmt::Debug for GenerateCeremonyReportUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerateCeremonyReportUseCase")
            .finish()
    }
}

impl GenerateCeremonyReportUseCase {
    #[must_use]
    pub fn new(
        instances: Arc<GetCeremonyInstanceUseCase>,
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        events: Arc<dyn CeremonyEventStorePort>,
    ) -> Self {
        Self {
            instances,
            definitions,
            events,
        }
    }

    #[tracing::instrument(name = "generate_ceremony_report", skip_all)]
    pub async fn execute(
        &self,
        input: GenerateCeremonyReportInput,
    ) -> Result<CeremonyReport, DomainError> {
        Self::check(&input)?;

        let mut sessions = Vec::with_capacity(input.ceremony_ids().len());
        for ceremony_id in input.ceremony_ids() {
            sessions.push(self.load(ceremony_id).await?);
        }

        let bindings = sessions
            .iter()
            .map(|session| {
                CeremonyReportBinding::new(
                    session.instance.id().clone(),
                    session.definition.name().clone(),
                    session.definition.version().clone(),
                    session.digest,
                    session.instance.bound_definition(),
                    session.completed,
                )
            })
            .collect();

        Ok(CeremonyReport::new(
            render_markdown(input.title(), &sessions)?,
            bindings,
        ))
    }

    /// What the request has to be before anything is read.
    ///
    /// The tool schemas say the same thing — `minItems`, `uniqueItems`,
    /// `minLength` — and the MCP server refuses a call that breaks them
    /// before any backend is reached. This is for the caller that
    /// speaks the RPC directly and never saw a schema.
    fn check(input: &GenerateCeremonyReportInput) -> Result<(), DomainError> {
        if input.ceremony_ids().is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "ceremony_report.ceremony_ids",
            });
        }
        let mut seen = BTreeSet::new();
        for ceremony_id in input.ceremony_ids() {
            if !seen.insert(ceremony_id) {
                return Err(DomainError::InvalidDocument {
                    reason: format!(
                        "the report names the ceremony `{ceremony_id}` more than once, \
                         and duplicates are refused"
                    ),
                });
            }
        }
        // A blank title is refused by `ReportTitle`, which is the only
        // way one reaches this input, so there is nothing left to check
        // here about it.
        Ok(())
    }

    /// One session, with everything the report quotes of it.
    async fn load(&self, ceremony_id: &CeremonyId) -> Result<ReportedSession, DomainError> {
        let instance = self.instances.execute(ceremony_id).await?;
        let definition = self.definitions.execute(&instance).await?;
        let completed = CeremonyInstanceView::project(&instance, &definition)?.is_completed();
        let digest = definition.digest()?;
        // The whole stream, from the first record: a journal that
        // starts in the middle is not one a reader can verify.
        let journal = self.events.read(ceremony_id, StreamVersion::EMPTY).await?;
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

    use made_core::entities::CeremonyInstance;
    use made_core::ports::CeremonySnapshotStorePort;
    use made_core::value_objects::{
        Attributes, AuditActorKind, CeremonyContext, StepOutput, StepResult,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_resolver, lease_owner, lease_ttl, now,
        started_instance, stream, DefinitionRepositoryFake, EventStoreFake, FixedClock,
        StepHandlerFake,
    };
    use crate::usecases::{ReportTitle, RunCeremonyInput, RunCeremonyUseCase};

    struct Fixture {
        usecase: GenerateCeremonyReportUseCase,
    }

    async fn fixture() -> Fixture {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let store = Arc::new(EventStoreFake::default());
        store.save(&started_instance(&definition)).await.unwrap();
        // A second session of the same definition, so a report over
        // two ids has something to keep in order.
        let other = CeremonyInstance::start(
            CeremonyId::new("session-2").unwrap(),
            &definition,
            CeremonyContext::empty(),
            now(),
        );
        store.save(&other).await.unwrap();

        Fixture {
            usecase: GenerateCeremonyReportUseCase::new(
                Arc::new(GetCeremonyInstanceUseCase::new(stream(store.clone()))),
                definition_resolver(definitions),
                store,
            ),
        }
    }

    fn ids(values: &[&str]) -> Vec<CeremonyId> {
        values
            .iter()
            .map(|value| CeremonyId::new(*value).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn a_report_projects_the_session_its_definition_and_its_stream() {
        let fixture = fixture().await;

        let report = fixture
            .usecase
            .execute(GenerateCeremonyReportInput::new(
                vec![ceremony_id()],
                Some(ReportTitle::new("Session review").unwrap()),
            ))
            .await
            .unwrap();

        assert_eq!(report.ceremony_count(), 1);
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
        assert!(report.markdown().contains("ceremony_instance_started"));

        let binding = &report.bindings()[0];
        assert_eq!(binding.ceremony_id(), &ceremony_id());
        assert_eq!(binding.definition_name(), definition().name());
        assert!(!binding.completed());
        // Started from a document, not from a published version.
        assert!(binding.bound_definition_digest().is_none());
    }

    /// Read-only means asking twice answers the same bytes. A report
    /// that drifted between two reads of one unchanged session would
    /// not be a projection of it.
    #[tokio::test]
    async fn the_same_state_reports_the_same_bytes() {
        let fixture = fixture().await;
        let input = GenerateCeremonyReportInput::new(ids(&["session-2"]), None);

        let first = fixture.usecase.execute(input.clone()).await.unwrap();
        let second = fixture.usecase.execute(input).await.unwrap();

        assert_eq!(first, second);
        assert!(first.markdown().starts_with("# Ceremony report\n\n"));
    }

    #[tokio::test]
    async fn the_report_keeps_the_order_the_caller_asked_for() {
        let fixture = fixture().await;

        let report = fixture
            .usecase
            .execute(GenerateCeremonyReportInput::new(
                ids(&["session-2", "ceremony-1"]),
                None,
            ))
            .await
            .unwrap();

        assert_eq!(
            report
                .bindings()
                .iter()
                .map(|binding| binding.ceremony_id().as_str().to_owned())
                .collect::<Vec<_>>(),
            ["session-2", "ceremony-1"]
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
        RunCeremonyUseCase::new(
            definitions.clone(),
            stream(store.clone()),
            Arc::new(StepHandlerFake::succeeding(
                StepResult::completed(output).unwrap(),
            )),
            Arc::new(FixedClock::new(now())),
        )
        .execute(RunCeremonyInput::new(
            ceremony_id(),
            definition,
            CeremonyContext::empty(),
            lease_owner(),
            lease_ttl(),
            "operator-1",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();

        // Every cached fold, gone. The stream is the whole of what is
        // left, and the report is asked the same question.
        store.forget(&ceremony_id()).await.unwrap();
        let usecase = GenerateCeremonyReportUseCase::new(
            Arc::new(GetCeremonyInstanceUseCase::new(stream(store.clone()))),
            definition_resolver(definitions),
            store,
        );

        let report = usecase
            .execute(GenerateCeremonyReportInput::new(vec![ceremony_id()], None))
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
            .execute(GenerateCeremonyReportInput::new(ids(&["missing"]), None))
            .await
            .unwrap_err();

        assert!(matches!(error, DomainError::NotFound { .. }));
    }

    /// The tool schemas say the same thing and the MCP server enforces
    /// it before any backend is reached. These are for the caller that
    /// speaks the RPC directly.
    #[tokio::test]
    async fn an_unreportable_request_is_refused_before_anything_is_read() {
        let fixture = fixture().await;

        assert!(matches!(
            fixture
                .usecase
                .execute(GenerateCeremonyReportInput::new(Vec::new(), None))
                .await
                .unwrap_err(),
            DomainError::EmptyCollection {
                field: "ceremony_report.ceremony_ids"
            }
        ));
        let duplicate = fixture
            .usecase
            .execute(GenerateCeremonyReportInput::new(
                ids(&["session-2", "session-2"]),
                None,
            ))
            .await
            .unwrap_err();
        assert!(
            matches!(&duplicate, DomainError::InvalidDocument { reason } if reason.contains("duplicate")),
            "{duplicate:?}"
        );
        // A blank heading never reaches the use case: it is refused
        // where a title is built, which is the one place both arms
        // build one.
        assert!(matches!(
            ReportTitle::new("   "),
            Err(DomainError::EmptyField { field: "title" })
        ));
    }
}
