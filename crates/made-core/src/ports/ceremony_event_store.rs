//! [`CeremonyEventStorePort`] — a ceremony is its event stream.
//!
//! The engine owns what a stream is: one per ceremony, sequences from
//! 1 with no gaps, each record sealed against the one before it, an
//! event id that occurs once. The host owns where the bytes go. The
//! contract is checked by a conformance suite rather than assumed, and
//! the part of it that does not depend on storage — how a batch of
//! facts becomes the records that continue a stream — is written once
//! here, in [`seal_continuation`], so every adapter refuses the same
//! things for the same reasons.

use std::collections::HashSet;

use async_trait::async_trait;

use crate::entities::{AuditFact, AuditRecord};
use crate::error::DomainError;
use crate::ports::{AppendOutcome, PositionedRecord};
use crate::value_objects::{CeremonyEventPageLimit, CeremonyId, GlobalPosition, StreamVersion};

#[async_trait]
pub trait CeremonyEventStorePort: Send + Sync {
    /// Seal `facts` as the records that continue `stream` and land them
    /// together, or land nothing.
    ///
    /// `expected` is the version the facts were decided against and is
    /// checked inside the same transaction that writes: a stale
    /// expectation is an [`AppendOutcome::Conflict`], not an error. An
    /// empty batch, a fact from another ceremony, or an event id the
    /// stream already holds — or that the batch holds twice — is a
    /// [`DomainError`], and nothing of the batch lands either way.
    async fn append(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError>;

    /// The records of `stream` with a sequence above `after`, in order.
    /// `StreamVersion::EMPTY` reads the whole stream.
    async fn read(
        &self,
        stream: &CeremonyId,
        after: StreamVersion,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError>;

    /// At most `limit` records of every stream, in global order,
    /// starting at `from` inclusive.
    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError>;

    /// The version of `stream`: `StreamVersion::EMPTY` when nothing was
    /// ever appended to it.
    async fn head(&self, stream: &CeremonyId) -> Result<StreamVersion, DomainError>;

    /// Every stream the store holds, each once, sorted by id.
    async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError>;
}

/// Seal `facts` as the records that follow `existing`, the whole of
/// `stream` as stored.
///
/// Pure: it decides what the records would be and refuses what the
/// contract refuses, without touching a store. An adapter calls it
/// inside its transaction once the expectation has been checked, and
/// writes the result or nothing.
pub fn seal_continuation(
    stream: &CeremonyId,
    existing: &[AuditRecord],
    facts: Vec<AuditFact>,
) -> Result<Vec<AuditRecord>, DomainError> {
    if facts.is_empty() {
        return Err(DomainError::EmptyCollection {
            field: "event_store_facts",
        });
    }
    if facts.iter().any(|fact| &fact.ceremony_id != stream) {
        return Err(DomainError::InvariantViolated {
            reason: "an event store append only takes facts of the stream it targets",
        });
    }

    let mut seen: HashSet<_> = existing.iter().map(AuditRecord::event_id).collect();
    for fact in &facts {
        if !seen.insert(&fact.event_id) {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_event",
            });
        }
    }

    let mut head = existing.last().cloned();
    let mut sealed = Vec::with_capacity(facts.len());
    for fact in facts {
        let record = match &head {
            Some(previous) => AuditRecord::following(fact, previous)?,
            None => AuditRecord::first(fact)?,
        };
        head = Some(record.clone());
        sealed.push(record);
    }
    Ok(sealed)
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;
    use crate::entities::ceremony_events::CeremonyCompleted;
    use crate::entities::{AuditChain, CeremonyEvent};
    use crate::value_objects::{
        AuditActor, AuditActorKind, CeremonyName, CeremonyVersion, EventId, StateId,
    };

    fn fact(stream: &str, event: &str) -> AuditFact {
        AuditFact {
            event_id: EventId::new(event).unwrap(),
            event: CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
                final_state: StateId::new("DONE").unwrap(),
                completed_at: OffsetDateTime::UNIX_EPOCH,
            }),
            ceremony_id: CeremonyId::new(stream).unwrap(),
            definition_name: CeremonyName::new("sealing").unwrap(),
            definition_version: CeremonyVersion::v1(),
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            actor: AuditActor::new("test", AuditActorKind::Engine, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        }
    }

    fn stream() -> CeremonyId {
        CeremonyId::new("c1").unwrap()
    }

    #[test]
    fn a_batch_continues_the_existing_records_as_one_chain() {
        let first = seal_continuation(&stream(), &[], vec![fact("c1", "e1")]).unwrap();
        let rest =
            seal_continuation(&stream(), &first, vec![fact("c1", "e2"), fact("c1", "e3")]).unwrap();

        let all: Vec<_> = first.into_iter().chain(rest).collect();
        assert_eq!(
            all.iter().map(|r| r.sequence().value()).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert!(AuditChain::verify(&all).is_intact());
    }

    #[test]
    fn an_empty_batch_is_refused() {
        assert!(matches!(
            seal_continuation(&stream(), &[], Vec::new()),
            Err(DomainError::EmptyCollection { .. })
        ));
    }

    #[test]
    fn a_fact_of_another_ceremony_is_refused() {
        assert!(matches!(
            seal_continuation(&stream(), &[], vec![fact("c1", "e1"), fact("c2", "e2")]),
            Err(DomainError::InvariantViolated { .. })
        ));
    }

    #[test]
    fn a_duplicate_event_id_is_refused_within_a_batch_and_against_the_stream() {
        assert!(matches!(
            seal_continuation(&stream(), &[], vec![fact("c1", "e1"), fact("c1", "e1")]),
            Err(DomainError::AlreadyExists { .. })
        ));

        let existing = seal_continuation(&stream(), &[], vec![fact("c1", "e1")]).unwrap();
        assert!(matches!(
            seal_continuation(&stream(), &existing, vec![fact("c1", "e1")]),
            Err(DomainError::AlreadyExists { .. })
        ));
    }
}
