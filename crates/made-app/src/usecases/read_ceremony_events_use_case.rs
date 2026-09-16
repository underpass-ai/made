use std::fmt;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::StreamVersion;

use super::{CeremonyEventPage, ReadCeremonyEventsInput};

/// Reads one page of a ceremony's event stream.
///
/// ADR-012: a ceremony *is* its stream, and the sealed records are the
/// evidence a session leaves behind. Handing them out whole — digests
/// and hash chain included — is what lets a caller run
/// `AuditChain::verify` on what it received rather than trusting the
/// engine that sent it.
pub struct ReadCeremonyEventsUseCase {
    events: Arc<dyn CeremonyEventStorePort>,
}

impl fmt::Debug for ReadCeremonyEventsUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("ReadCeremonyEventsUseCase").finish()
    }
}

impl ReadCeremonyEventsUseCase {
    #[must_use]
    pub fn new(events: Arc<dyn CeremonyEventStorePort>) -> Self {
        Self { events }
    }

    #[tracing::instrument(
        name = "read_ceremony_events",
        skip_all,
        fields(ceremony_id = %input.ceremony_id())
    )]
    pub async fn execute(
        &self,
        input: ReadCeremonyEventsInput,
    ) -> Result<CeremonyEventPage, DomainError> {
        let head = self.events.head(input.ceremony_id()).await?;
        // A stream nothing was ever appended to is a ceremony that was
        // never started. Answering with an empty page would tell a
        // caller it is caught up on a session that does not exist.
        if head.is_empty() {
            return Err(DomainError::NotFound {
                what: "ceremony_instance",
            });
        }

        let mut records = self
            .events
            .read(input.ceremony_id(), input.from_version())
            .await?;
        records.truncate(input.limit());

        let next = records.last().map_or(head, |record| {
            StreamVersion::from_sequence(record.sequence())
        });
        Ok(CeremonyEventPage::new(records, next, head))
    }
}

#[cfg(test)]
mod tests {
    use made_core::entities::AuditChain;
    use made_core::value_objects::{AuditActorKind, CeremonyId, StepOutput, StepResult};
    use std::sync::Arc;

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_resolver, idempotency_key, lease_owner, lease_ttl, now,
        role_id, started_instance, step_id, stream, DefinitionRepositoryFake, EventStoreFake,
        FixedClock,
    };
    use crate::usecases::{
        CompleteCeremonyStepInput, CompleteCeremonyStepUseCase, StartCeremonyStepInput,
        StartCeremonyStepUseCase,
    };

    /// A session with three records: it was opened, a step was taken on
    /// and the step ended. Three is the smallest stream a page can be
    /// smaller than.
    async fn three_record_stream() -> Arc<EventStoreFake> {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let store = Arc::new(EventStoreFake::default());
        store.save(&started_instance(&definition)).await.unwrap();

        let clock = Arc::new(FixedClock::new(now()));
        StartCeremonyStepUseCase::new(
            definition_resolver(definitions.clone()),
            stream(store.clone()),
            clock.clone(),
        )
        .execute(StartCeremonyStepInput::new(
            ceremony_id(),
            role_id(),
            AuditActorKind::Agent,
            step_id(),
            lease_owner(),
            idempotency_key("lease-1"),
            lease_ttl(),
        ))
        .await
        .unwrap();
        CompleteCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream(store.clone()),
            clock,
        )
        .execute(CompleteCeremonyStepInput::new(
            ceremony_id(),
            step_id(),
            StepResult::completed(StepOutput::empty()).unwrap(),
            AuditActorKind::Agent,
        ))
        .await
        .unwrap();
        store
    }

    fn read_events(store: Arc<EventStoreFake>) -> ReadCeremonyEventsUseCase {
        ReadCeremonyEventsUseCase::new(store)
    }

    #[tokio::test]
    async fn the_whole_stream_is_one_page_when_the_limit_allows_it() {
        let store = three_record_stream().await;

        let page = read_events(store)
            .execute(ReadCeremonyEventsInput::new(
                ceremony_id(),
                StreamVersion::EMPTY,
                None,
            ))
            .await
            .unwrap();

        assert_eq!(page.records().len(), 3);
        assert_eq!(page.next_version(), StreamVersion::new(3));
        assert_eq!(page.head_version(), StreamVersion::new(3));
        assert!(!page.has_more());
        // What the caller received is verifiable on its own: the point
        // of handing out sealed records rather than a rendering of them.
        assert!(AuditChain::verify(page.records()).is_intact());
    }

    #[tokio::test]
    async fn a_limit_is_respected_and_the_next_version_continues_the_read() {
        let store = three_record_stream().await;
        let usecase = read_events(store);

        let first = usecase
            .execute(ReadCeremonyEventsInput::new(
                ceremony_id(),
                StreamVersion::EMPTY,
                Some(2),
            ))
            .await
            .unwrap();

        assert_eq!(first.records().len(), 2);
        assert_eq!(first.next_version(), StreamVersion::new(2));
        assert_eq!(first.head_version(), StreamVersion::new(3));
        assert!(first.has_more());

        let second = usecase
            .execute(ReadCeremonyEventsInput::new(
                ceremony_id(),
                first.next_version(),
                Some(2),
            ))
            .await
            .unwrap();

        assert_eq!(second.records().len(), 1);
        assert_eq!(second.next_version(), StreamVersion::new(3));
        assert!(!second.has_more());
        // Read twice, nothing seen twice and nothing missed: the two
        // pages are the stream.
        assert_eq!(
            first
                .records()
                .iter()
                .chain(second.records())
                .map(|record| record.sequence().value())
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }

    /// A reader past the end is caught up, not wrong. It gets an empty
    /// page and the head, so the next read asks for what it has not
    /// seen rather than starting over.
    #[tokio::test]
    async fn a_version_past_the_head_reads_nothing_and_answers_with_the_head() {
        let store = three_record_stream().await;

        let page = read_events(store)
            .execute(ReadCeremonyEventsInput::new(
                ceremony_id(),
                StreamVersion::new(99),
                None,
            ))
            .await
            .unwrap();

        assert!(page.records().is_empty());
        assert_eq!(page.next_version(), StreamVersion::new(3));
        assert_eq!(page.head_version(), StreamVersion::new(3));
        assert!(!page.has_more());
    }

    #[tokio::test]
    async fn a_ceremony_with_no_stream_is_not_found() {
        let store = three_record_stream().await;

        let error = read_events(store)
            .execute(ReadCeremonyEventsInput::new(
                CeremonyId::new("never-started").unwrap(),
                StreamVersion::EMPTY,
                None,
            ))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            DomainError::NotFound {
                what: "ceremony_instance"
            }
        ));
    }
}
