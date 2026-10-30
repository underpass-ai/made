use std::fmt;
use std::sync::Arc;

use made_core::entities::AuditChain;
use made_core::error::DomainError;
use made_core::ports::ProtocolEventStorePort;
use made_core::value_objects::{ProtocolId, StreamVersion};

use super::{ProtocolJournalVerdict, ReadWholeProtocolEventsUseCase};

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
pub struct VerifyProtocolJournalUseCase {
    events: Arc<dyn ProtocolEventStorePort>,
}

impl fmt::Debug for VerifyProtocolJournalUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifyProtocolJournalUseCase")
            .finish()
    }
}

impl VerifyProtocolJournalUseCase {
    #[must_use]
    pub fn new(events: Arc<dyn ProtocolEventStorePort>) -> Self {
        Self { events }
    }

    #[tracing::instrument(
        name = "verify_protocol_journal",
        skip_all,
        fields(protocol_id = %protocol_id)
    )]
    pub async fn execute(
        &self,
        protocol_id: &ProtocolId,
    ) -> Result<ProtocolJournalVerdict, DomainError> {
        let records = ReadWholeProtocolEventsUseCase::new(self.events.clone())
            .execute(protocol_id)
            .await?;
        // A stream nothing was ever appended to is a session that was
        // never started. Calling that intact would answer a question
        // about a protocol that does not exist with a reassurance.
        if records.is_empty() {
            return Err(DomainError::NotFound {
                what: "protocol_instance",
            });
        }
        let head = records.last().map_or(StreamVersion::EMPTY, |record| {
            StreamVersion::from_sequence(record.sequence())
        });
        Ok(ProtocolJournalVerdict::new(
            protocol_id.clone(),
            head,
            records.len(),
            AuditChain::verify(&records),
        ))
    }
}

#[cfg(test)]
mod tests {
    use made_core::entities::{AuditFact, AuditRecord, ProtocolEvent};
    use made_core::ports::AppendOutcome;
    use made_core::value_objects::{AuditActorKind, AuditSequence};

    use super::*;
    use crate::services::session_facts;
    use crate::usecases::protocol_test_support::{
        definition, now, protocol_id, started_instance, EventStoreFake,
    };

    /// A store that hands back whatever records it was given, however
    /// broken. The point of a verifier is what it says about a journal
    /// no honest store would have written.
    #[derive(Debug)]
    struct TamperedStore {
        records: Vec<AuditRecord>,
    }

    #[async_trait::async_trait]
    impl ProtocolEventStorePort for TamperedStore {
        async fn append(
            &self,
            _stream: &ProtocolId,
            _expected: StreamVersion,
            _facts: Vec<AuditFact>,
        ) -> Result<AppendOutcome, DomainError> {
            unreachable!("verification never writes")
        }

        async fn read(
            &self,
            _stream: &ProtocolId,
            after: StreamVersion,
            limit: made_core::value_objects::ProtocolEventPageLimit,
        ) -> Result<Vec<AuditRecord>, DomainError> {
            Ok(self
                .records
                .iter()
                .filter(|record| record.sequence().value() > after.value())
                .take(limit.value())
                .cloned()
                .collect())
        }

        async fn read_all(
            &self,
            _from: made_core::value_objects::GlobalPosition,
            _limit: made_core::value_objects::ProtocolEventPageLimit,
        ) -> Result<Vec<made_core::ports::PositionedRecord>, DomainError> {
            unreachable!("verification reads one stream")
        }

        async fn head(&self, _stream: &ProtocolId) -> Result<StreamVersion, DomainError> {
            Ok(self.records.last().map_or(StreamVersion::EMPTY, |record| {
                StreamVersion::from_sequence(record.sequence())
            }))
        }

        async fn streams(&self) -> Result<Vec<ProtocolId>, DomainError> {
            unreachable!("verification is asked about one stream")
        }
    }

    fn fact(ordinal: u64) -> AuditFact {
        let instance = started_instance(&definition());
        let event = ProtocolEvent::ProtocolCompleted(
            made_core::entities::protocol_events::ProtocolCompleted {
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

    fn verifier(records: Vec<AuditRecord>) -> VerifyProtocolJournalUseCase {
        VerifyProtocolJournalUseCase::new(Arc::new(TamperedStore { records }))
    }

    #[tokio::test]
    async fn a_whole_journal_is_intact_and_names_its_head() {
        let verdict = verifier(chain(3)).execute(&protocol_id()).await.unwrap();

        assert!(verdict.is_intact());
        assert_eq!(verdict.protocol_id(), &protocol_id());
        assert_eq!(verdict.head_version(), StreamVersion::new(3));
        assert_eq!(verdict.record_count(), 3);
        assert_eq!(verdict.first_broken_sequence(), None);
        assert_eq!(verdict.reason(), None);
    }

    #[tokio::test]
    async fn a_record_removed_from_the_middle_is_named_with_its_position() {
        let whole = chain(3);
        let gapped = vec![whole[0].clone(), whole[2].clone()];

        let verdict = verifier(gapped).execute(&protocol_id()).await.unwrap();

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

        let verdict = verifier(records).execute(&protocol_id()).await.unwrap();

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
            .execute(&protocol_id())
            .await
            .unwrap_err();

        assert!(
            matches!(
                error,
                DomainError::NotFound {
                    what: "protocol_instance"
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
            .read(
                instance.id(),
                StreamVersion::EMPTY,
                made_core::value_objects::ProtocolEventPageLimit::DEFAULT,
            )
            .await
            .unwrap();

        let verdict = verifier(records).execute(instance.id()).await.unwrap();

        assert!(verdict.is_intact());
        assert_eq!(verdict.record_count(), 1);
    }
}
