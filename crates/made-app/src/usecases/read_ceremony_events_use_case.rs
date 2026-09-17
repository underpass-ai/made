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
        let mut records = self
            .events
            .read(input.ceremony_id(), input.from_version())
            .await?;
        // The head **after** the page, never before it. The two reads
        // are not one instant, and a session someone else is still
        // driving grows between them: sampled first, the head could be
        // older than the last record handed out, and the answer said
        // `next_version > head_version` with `has_more() == false` —
        // caught up, and past the end of the stream it was caught up
        // with. Read afterwards, the head is at least what was read, so
        // a concurrent append shows up as more to read rather than as
        // an impossible answer.
        //
        // Reading the page first also makes the not-found check truthful
        // for a stream that was started between the two calls.
        //
        // #70 owns the two things this cannot fix here: a `limit` at the
        // port, so a page is not read whole and then truncated, and a
        // head that does not cost a scan.
        let head = self.events.head(input.ceremony_id()).await?;
        // A stream nothing was ever appended to is a ceremony that was
        // never started. Answering with an empty page would tell a
        // caller it is caught up on a session that does not exist.
        if head.is_empty() {
            return Err(DomainError::NotFound {
                what: "ceremony_instance",
            });
        }

        records.truncate(input.limit());

        let next = records.last().map_or(head, |record| {
            StreamVersion::from_sequence(record.sequence())
        });
        Ok(CeremonyEventPage::new(records, next, head))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use made_core::entities::{AuditChain, AuditFact, AuditRecord};
    use made_core::ports::{AppendOutcome, CeremonyEventStorePort, PositionedRecord};
    use made_core::value_objects::{AuditActorKind, CeremonyId, StepOutput, StepResult};
    use made_core::value_objects::{EventId, GlobalPosition};
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
            .execute(
                ReadCeremonyEventsInput::new(ceremony_id(), StreamVersion::EMPTY, None).unwrap(),
            )
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
            .execute(
                ReadCeremonyEventsInput::new(ceremony_id(), StreamVersion::EMPTY, Some(2)).unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(first.records().len(), 2);
        assert_eq!(first.next_version(), StreamVersion::new(2));
        assert_eq!(first.head_version(), StreamVersion::new(3));
        assert!(first.has_more());

        let second = usecase
            .execute(
                ReadCeremonyEventsInput::new(ceremony_id(), first.next_version(), Some(2)).unwrap(),
            )
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
            .execute(
                ReadCeremonyEventsInput::new(ceremony_id(), StreamVersion::new(99), None).unwrap(),
            )
            .await
            .unwrap();

        assert!(page.records().is_empty());
        assert_eq!(page.next_version(), StreamVersion::new(3));
        assert_eq!(page.head_version(), StreamVersion::new(3));
        assert!(!page.has_more());
    }

    /// Somebody else was still driving the session while it was being
    /// read.
    ///
    /// The wrapper appends one record during the `read` call, which is
    /// exactly the window the two calls leave open. Sampling the head
    /// first, the answer came back with `next_version` past
    /// `head_version` and `has_more()` false at the same time: caught
    /// up, and past the end of what it was caught up with. There is no
    /// reading of that a client can act on.
    #[tokio::test]
    async fn an_append_during_the_read_leaves_the_head_at_least_at_the_page() {
        let store = three_record_stream().await;
        let interposing = Arc::new(AppendsDuringTheRead::over(store));

        let page = ReadCeremonyEventsUseCase::new(interposing.clone())
            .execute(
                ReadCeremonyEventsInput::new(ceremony_id(), StreamVersion::EMPTY, None).unwrap(),
            )
            .await
            .unwrap();

        assert!(
            interposing.appended(),
            "the wrapper must have written during the read; otherwise this proves nothing"
        );
        assert_eq!(page.records().len(), 4);
        assert!(
            page.next_version() <= page.head_version(),
            "next_version {:?} is past head_version {:?}",
            page.next_version(),
            page.head_version()
        );
        assert!(
            !page.has_more(),
            "everything the store held was read, so there is nothing further to fetch"
        );
    }

    /// A store that somebody else writes to in the middle of a read.
    ///
    /// It appends once, on the first `read`, before answering — the one
    /// moment a concurrent writer can land between a page and a head.
    /// The record it writes is the stream's own last one again under a
    /// fresh event id, so what lands is a record the chain accepts
    /// rather than something invented for the test.
    struct AppendsDuringTheRead {
        inner: Arc<EventStoreFake>,
        appended: AtomicBool,
    }

    impl AppendsDuringTheRead {
        fn over(inner: Arc<EventStoreFake>) -> Self {
            Self {
                inner,
                appended: AtomicBool::new(false),
            }
        }

        fn appended(&self) -> bool {
            self.appended.load(Ordering::SeqCst)
        }

        async fn append_one_more(&self, stream: &CeremonyId) {
            let head = self.inner.head(stream).await.unwrap();
            let last = self
                .inner
                .read(stream, StreamVersion::EMPTY)
                .await
                .unwrap()
                .pop()
                .expect("the stream this wrapper is used over is not empty");
            let fact = AuditFact {
                event_id: EventId::new(format!("{}:concurrent", last.event_id())).unwrap(),
                event: last.event().cloned().expect("the record carries its event"),
                ceremony_id: last.ceremony_id().clone(),
                definition_name: last.definition_name().clone(),
                definition_version: last.definition_version().clone(),
                occurred_at: last.occurred_at(),
                actor: last.actor().clone(),
                correlation_id: last.correlation_id().cloned(),
                causation_id: last.causation_id().cloned(),
                trace: None,
            };
            self.inner.append(stream, head, vec![fact]).await.unwrap();
        }
    }

    #[async_trait::async_trait]
    impl CeremonyEventStorePort for AppendsDuringTheRead {
        async fn append(
            &self,
            stream: &CeremonyId,
            expected: StreamVersion,
            facts: Vec<AuditFact>,
        ) -> Result<AppendOutcome, DomainError> {
            self.inner.append(stream, expected, facts).await
        }

        async fn read(
            &self,
            stream: &CeremonyId,
            after: StreamVersion,
        ) -> Result<Vec<AuditRecord>, DomainError> {
            if !self.appended.swap(true, Ordering::SeqCst) {
                self.append_one_more(stream).await;
            }
            self.inner.read(stream, after).await
        }

        async fn read_all(
            &self,
            from: GlobalPosition,
            limit: usize,
        ) -> Result<Vec<PositionedRecord>, DomainError> {
            self.inner.read_all(from, limit).await
        }

        async fn head(&self, stream: &CeremonyId) -> Result<StreamVersion, DomainError> {
            self.inner.head(stream).await
        }

        async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError> {
            self.inner.streams().await
        }
    }

    #[tokio::test]
    async fn a_ceremony_with_no_stream_is_not_found() {
        let store = three_record_stream().await;

        let error = read_events(store)
            .execute(
                ReadCeremonyEventsInput::new(
                    CeremonyId::new("never-started").unwrap(),
                    StreamVersion::EMPTY,
                    None,
                )
                .unwrap(),
            )
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
