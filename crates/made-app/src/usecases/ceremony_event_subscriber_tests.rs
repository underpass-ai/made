//! What a projection is told, and when (plan §3.1 A5).
//!
//! The seam is `CeremonyEventSubscriberPort` and the only thing that
//! notifies it is `SessionStream`. These are the three things every
//! projection built on it relies on, driven through real use cases
//! rather than by calling the stream directly: what the engine seals
//! is what a subscriber sees.

use std::sync::Arc;

use async_trait::async_trait;
use made_core::ports::{CeremonyEventSubscriberPort, PositionedRecord};
use made_core::value_objects::{AuditActorKind, StepOutput, StepResult};
use tokio::sync::RwLock;

use super::ceremony_test_support::{
    ceremony_id, definition, definition_resolver, idempotency_key, lease_owner, lease_ttl, now,
    role_id, started_instance, step_id, stream_conflicting_once_watched_by,
    stream_losing_every_race_watched_by, stream_watched_by, trigger, DefinitionRepositoryFake,
    EventStoreFake, FixedClock,
};
use super::{
    ApplyCeremonyTransitionInput, ApplyCeremonyTransitionUseCase, CompleteCeremonyStepInput,
    CompleteCeremonyStepUseCase, StartCeremonyStepInput, StartCeremonyStepUseCase,
};
use crate::services::SessionStream;

/// Every notification, flattened, in the order it arrived.
#[derive(Debug, Default)]
struct Watcher {
    observed: RwLock<Vec<PositionedRecord>>,
    notifications: RwLock<Vec<usize>>,
}

#[async_trait]
impl CeremonyEventSubscriberPort for Watcher {
    async fn observe(&self, records: &[PositionedRecord]) {
        self.notifications.write().await.push(records.len());
        self.observed.write().await.extend_from_slice(records);
    }
}

impl Watcher {
    /// `(event type, sequence, position)` of everything observed.
    async fn seen(&self) -> Vec<(String, u64, u64)> {
        self.observed
            .read()
            .await
            .iter()
            .map(|entry| {
                (
                    entry.record.event_type().as_str().to_owned(),
                    entry.record.sequence().value(),
                    entry.position.value(),
                )
            })
            .collect()
    }

    async fn notifications(&self) -> Vec<usize> {
        self.notifications.read().await.clone()
    }
}

/// The store, the watcher and a session already opened on it.
async fn watched() -> (Arc<EventStoreFake>, Arc<Watcher>) {
    let store = Arc::new(EventStoreFake::default());
    store.save(&started_instance(&definition())).await.unwrap();
    (store, Arc::new(Watcher::default()))
}

fn claim(stream: Arc<SessionStream>) -> StartCeremonyStepUseCase {
    StartCeremonyStepUseCase::new(
        definition_resolver(Arc::new(DefinitionRepositoryFake::new(definition()))),
        stream,
        Arc::new(FixedClock::new(now())),
    )
}

fn claim_input() -> StartCeremonyStepInput {
    StartCeremonyStepInput::new(
        ceremony_id(),
        role_id(),
        AuditActorKind::Agent,
        step_id(),
        lease_owner(),
        idempotency_key("seam-1"),
        lease_ttl(),
    )
}

fn finish_input() -> CompleteCeremonyStepInput {
    CompleteCeremonyStepInput::new(
        ceremony_id(),
        step_id(),
        StepResult::completed(StepOutput::empty()).unwrap(),
        AuditActorKind::Agent,
    )
}

/// Claim, complete, transition: four events over three appends, each
/// seen once, in the order they were sealed, at the positions the
/// store filed them.
#[tokio::test]
async fn every_record_of_every_append_is_observed_once_and_in_order() {
    let (store, watcher) = watched().await;
    let stream = stream_watched_by(store.clone(), watcher.clone());
    let definitions = Arc::new(DefinitionRepositoryFake::new(definition()));

    claim(stream.clone()).execute(claim_input()).await.unwrap();
    CompleteCeremonyStepUseCase::new(
        definition_resolver(definitions.clone()),
        stream.clone(),
        Arc::new(FixedClock::new(now())),
    )
    .execute(finish_input())
    .await
    .unwrap();
    ApplyCeremonyTransitionUseCase::new(
        definition_resolver(definitions),
        stream,
        Arc::new(FixedClock::new(now())),
    )
    .execute(ApplyCeremonyTransitionInput::new(
        ceremony_id(),
        role_id(),
        AuditActorKind::Human,
        trigger(),
    ))
    .await
    .unwrap();

    // The fixture's opening was seeded, not appended through a
    // watched stream, so the sequences start at the claim.
    assert_eq!(
        watcher.seen().await,
        [
            ("step_started".to_owned(), 2, 2),
            ("step_completed".to_owned(), 3, 3),
            ("transition_applied".to_owned(), 4, 4),
            ("ceremony_completed".to_owned(), 5, 5),
        ]
    );
    // Three appends, and the move and the ending arrived together.
    assert_eq!(watcher.notifications().await, [1, 1, 2]);
}

/// A lost race is decided again, and the subscriber hears about the
/// attempt that landed — once, with what is actually in the stream.
#[tokio::test]
async fn an_append_that_conflicts_and_is_retried_is_observed_once() {
    let (store, watcher) = watched().await;
    let stream = stream_conflicting_once_watched_by(store.clone(), watcher.clone());

    claim(stream).execute(claim_input()).await.unwrap();

    assert_eq!(watcher.notifications().await, [1]);
    assert_eq!(watcher.seen().await, [("step_started".to_owned(), 2, 2)]);
    // What was observed is what the stream holds.
    let records = store.records(&ceremony_id()).await;
    assert_eq!(records.len(), 2);
    assert_eq!(
        watcher.observed.read().await[0].record,
        records[1],
        "a subscriber was told about a record the stream does not hold"
    );
}

/// An append that never lands seals nothing, so there is nothing to
/// be told about.
#[tokio::test]
async fn an_append_that_never_lands_is_not_observed() {
    let (store, watcher) = watched().await;
    let stream = stream_losing_every_race_watched_by(store, watcher.clone());

    let refused = claim(stream).execute(claim_input()).await;

    assert!(refused.is_err(), "the append was expected to be refused");
    assert!(watcher.notifications().await.is_empty());
    assert!(watcher.seen().await.is_empty());
}

/// Memory is not the transaction (ADR-013, and the module doc of the
/// recorder): a session that cannot record what it decided still ran.
///
/// The property used to be held by a use case calling a recorder that
/// swallowed its own failures. It is now held by the seam: the
/// subscriber signature returns nothing, so there is no failure to
/// swallow and no way for a projection to reach the append.
mod memory_is_not_the_transaction {
    use made_core::error::DomainError;
    use made_core::ports::{MemoryWriteOutcome, MemoryWriterPort};
    use made_core::value_objects::{
        GuardName, MemoryCapabilities, MemoryCapability, MemoryScope, MemoryWrite,
    };

    use super::*;
    use crate::services::SessionMemoryRecorder;
    use crate::usecases::ceremony_test_support::approval_definition;
    use crate::usecases::{ApproveCeremonyGuardInput, ApproveCeremonyGuardUseCase};

    /// A backend that is there and refuses everything — the shape of a
    /// memory whose store is unreachable.
    #[derive(Debug)]
    struct UnreachableMemory;

    #[async_trait]
    impl MemoryWriterPort for UnreachableMemory {
        async fn remember(
            &self,
            _scope: &MemoryScope,
            _write: MemoryWrite,
            _idempotency_key: &str,
        ) -> Result<MemoryWriteOutcome, DomainError> {
            Err(DomainError::InvariantViolated {
                reason: "the memory backend is unreachable",
            })
        }

        fn capabilities(&self) -> MemoryCapabilities {
            MemoryCapabilities::none().with(MemoryCapability::Remembering)
        }
    }

    #[tokio::test]
    async fn a_memory_that_refuses_every_write_does_not_fail_the_session() {
        let definition = approval_definition();
        let store = Arc::new(EventStoreFake::default());
        store.save(&started_instance(&definition)).await.unwrap();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition));
        let stream = stream_watched_by(
            store.clone(),
            Arc::new(SessionMemoryRecorder::new(
                Arc::new(UnreachableMemory),
                store.clone(),
            )),
        );

        let approved = ApproveCeremonyGuardUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        )
        .execute(ApproveCeremonyGuardInput::new(
            ceremony_id(),
            GuardName::new("human_approved").unwrap(),
            role_id(),
            AuditActorKind::Human,
        ))
        .await
        .expect("a memory that cannot be written to must not fail the session");

        assert_eq!(approved.guard_approvals().len(), 1);
        // And the approval is sealed, whatever memory did with it.
        assert_eq!(store.records(&ceremony_id()).await.len(), 2);
    }
}
