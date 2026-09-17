use std::fmt;
use std::sync::Arc;

use made_core::entities::AuditChain;
use made_core::error::DomainError;
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{CeremonyId, StreamVersion};

use super::CeremonyJournalVerdict;

/// Verifies the hash chain of one session's journal.
///
/// The verifier has existed since ADR-003 and had no caller: a chain
/// nobody checks is a claim, not evidence. This is the caller. It
/// reads the whole stream from its first record — a journal that
/// starts in the middle cannot be verified, only believed — and hands
/// the records to [`AuditChain`], which depends on nothing but the
/// bytes that were written.
///
/// It answers the same way whichever edition runs it, so an operator
/// who suspects a store can ask the engine, and a host that would
/// rather not take the engine's word can read the records and run the
/// same verifier itself.
pub struct VerifyCeremonyJournalUseCase {
    events: Arc<dyn CeremonyEventStorePort>,
}

impl fmt::Debug for VerifyCeremonyJournalUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifyCeremonyJournalUseCase")
            .finish()
    }
}

impl VerifyCeremonyJournalUseCase {
    #[must_use]
    pub fn new(events: Arc<dyn CeremonyEventStorePort>) -> Self {
        Self { events }
    }

    #[tracing::instrument(
        name = "verify_ceremony_journal",
        skip_all,
        fields(ceremony_id = %ceremony_id)
    )]
    pub async fn execute(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<CeremonyJournalVerdict, DomainError> {
        let records = self.events.read(ceremony_id, StreamVersion::EMPTY).await?;
        // A stream nothing was ever appended to is a session that was
        // never started. Calling that intact would answer a question
        // about a ceremony that does not exist with a reassurance.
        if records.is_empty() {
            return Err(DomainError::NotFound {
                what: "ceremony_instance",
            });
        }
        let head = records.last().map_or(StreamVersion::EMPTY, |record| {
            StreamVersion::from_sequence(record.sequence())
        });
        Ok(CeremonyJournalVerdict::new(
            ceremony_id.clone(),
            head,
            records.len(),
            AuditChain::verify(&records),
        ))
    }
}

#[cfg(test)]
mod tests {
    use made_core::entities::{AuditFact, AuditRecord, CeremonyEvent};
    use made_core::ports::AppendOutcome;
    use made_core::value_objects::{AuditActorKind, AuditSequence};

    use super::*;
    use crate::services::session_facts;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, now, started_instance, EventStoreFake,
    };

    /// A store that hands back whatever records it was given, however
    /// broken. The point of a verifier is what it says about a journal
    /// no honest store would have written.
    #[derive(Debug)]
    struct TamperedStore {
        records: Vec<AuditRecord>,
    }

    #[async_trait::async_trait]
    impl CeremonyEventStorePort for TamperedStore {
        async fn append(
            &self,
            _stream: &CeremonyId,
            _expected: StreamVersion,
            _facts: Vec<AuditFact>,
        ) -> Result<AppendOutcome, DomainError> {
            unreachable!("verification never writes")
        }

        async fn read(
            &self,
            _stream: &CeremonyId,
            _after: StreamVersion,
        ) -> Result<Vec<AuditRecord>, DomainError> {
            Ok(self.records.clone())
        }

        async fn read_all(
            &self,
            _from: made_core::value_objects::GlobalPosition,
            _limit: usize,
        ) -> Result<Vec<made_core::ports::PositionedRecord>, DomainError> {
            unreachable!("verification reads one stream")
        }

        async fn head(&self, _stream: &CeremonyId) -> Result<StreamVersion, DomainError> {
            unreachable!("verification reads the whole stream")
        }

        async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError> {
            unreachable!("verification is asked about one stream")
        }
    }

    fn fact(ordinal: u64) -> AuditFact {
        let instance = started_instance(&definition());
        let event = CeremonyEvent::CeremonyCompleted(
            made_core::entities::ceremony_events::CeremonyCompleted {
                final_state: made_core::value_objects::StateId::new("DONE").unwrap(),
                completed_at: now(),
            },
        );
        let mut fact = session_facts::fact(
            &instance,
            event,
            session_facts::party("verifier", AuditActorKind::Engine).unwrap(),
            now(),
        )
        .unwrap();
        fact.event_id = made_core::value_objects::EventId::new(format!("event-{ordinal}")).unwrap();
        fact
    }

    fn chain(length: u64) -> Vec<AuditRecord> {
        let mut records: Vec<AuditRecord> = Vec::new();
        for ordinal in 1..=length {
            let record = match records.last() {
                Some(previous) => AuditRecord::following(fact(ordinal), previous).unwrap(),
                None => AuditRecord::first(fact(ordinal)).unwrap(),
            };
            records.push(record);
        }
        records
    }

    fn verifier(records: Vec<AuditRecord>) -> VerifyCeremonyJournalUseCase {
        VerifyCeremonyJournalUseCase::new(Arc::new(TamperedStore { records }))
    }

    #[tokio::test]
    async fn a_whole_journal_is_intact_and_names_its_head() {
        let verdict = verifier(chain(3)).execute(&ceremony_id()).await.unwrap();

        assert!(verdict.is_intact());
        assert_eq!(verdict.ceremony_id(), &ceremony_id());
        assert_eq!(verdict.head_version(), StreamVersion::new(3));
        assert_eq!(verdict.record_count(), 3);
        assert_eq!(verdict.first_broken_sequence(), None);
        assert_eq!(verdict.reason(), None);
    }

    #[tokio::test]
    async fn a_record_removed_from_the_middle_is_named_with_its_position() {
        let whole = chain(3);
        let gapped = vec![whole[0].clone(), whole[2].clone()];

        let verdict = verifier(gapped).execute(&ceremony_id()).await.unwrap();

        assert!(!verdict.is_intact());
        assert_eq!(
            verdict.first_broken_sequence(),
            Some(AuditSequence::new(3).unwrap())
        );
        let reason = verdict.reason().expect("a broken chain says why");
        assert!(reason.contains('2') && reason.contains('3'), "{reason}");
        // The head is still what was handed over: a caller comparing
        // it with the record count sees the journal is short.
        assert_eq!(verdict.head_version(), StreamVersion::new(3));
        assert_eq!(verdict.record_count(), 2);
    }

    #[tokio::test]
    async fn an_altered_record_is_named_at_its_own_position() {
        let mut records = chain(3);
        let mut json = serde_json::to_value(&records[1]).unwrap();
        json["actor"]["actor_id"] = "someone-else".into();
        records[1] = serde_json::from_value(json).unwrap();

        let verdict = verifier(records).execute(&ceremony_id()).await.unwrap();

        assert_eq!(
            verdict.first_broken_sequence(),
            Some(AuditSequence::new(2).unwrap())
        );
        assert!(
            verdict.reason().unwrap().contains("digest"),
            "{:?}",
            verdict.reason()
        );
    }

    #[tokio::test]
    async fn a_session_with_no_stream_is_not_found_rather_than_intact() {
        let error = verifier(Vec::new())
            .execute(&ceremony_id())
            .await
            .unwrap_err();

        assert!(
            matches!(
                error,
                DomainError::NotFound {
                    what: "ceremony_instance"
                }
            ),
            "expected NotFound, got {error:?}"
        );
    }

    /// What the engine wrote verifies: the use case is not only
    /// correct about journals a test built by hand.
    #[tokio::test]
    async fn a_stream_the_engine_wrote_verifies() {
        let store = Arc::new(EventStoreFake::default());
        let instance = started_instance(&definition());
        store.save(&instance).await.unwrap();
        let records = store
            .read(instance.id(), StreamVersion::EMPTY)
            .await
            .unwrap();

        let verdict = verifier(records).execute(instance.id()).await.unwrap();

        assert!(verdict.is_intact());
        assert_eq!(verdict.record_count(), 1);
    }
}
