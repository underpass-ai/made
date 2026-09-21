//! A bound host asking for what it is owed.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    AckOutcome, BindOutcome, BindReplacement, CeremonyEventStorePort, CeremonyProgressNotifierPort,
    CeremonyProgressSubscriptionPort, DeliveryFailureOutcome, EnqueueOutcome,
    HostActivationOutcome, HostDeliveryFilter, HostDeliveryLedgerPort, HostDeliveryPage,
    HostDeliveryPageLimit, HostDeliveryQuery, IntegratorBindingPort, LeasedDelivery,
    ProcessedOutcome, RecordedActivation, SupersessionOutcome,
};
use made_core::value_objects::{
    AttentionEventId, AttentionKind, CeremonyId, DeliveryExpiryCause, DeliveryFailureReason,
    DurationMs, FollowReplacement, GlobalPosition, HostActivationMode, HostAddress,
    HostAgentIncarnation, HostDeliveryId, HostDeliveryItem, HostDeliveryLease, HostDeliveryLeaseId,
    HostDeliveryObservation, HostDeliveryPolicy, HostDeliveryRecord, HostDeliveryTarget,
    HostDestination, HostKind, IntegratorBinding, IntegratorBindingId, IntegratorFence,
    IntegratorScope, ProcessedActionRef, RoleId,
};
use time::OffsetDateTime;

use super::*;
use crate::usecases::ceremony_test_support::{
    attention_recovery, ceremony_id, definition, definition_resolver, now,
    store_with_a_sealed_step_result, stream, DefinitionRepositoryFake, EventStoreFake, FixedClock,
};
use crate::usecases::integrator::AwaitIntegratorAttentionInput;

const INTEGRATOR: &str = "INTEGRATOR";

fn binding() -> IntegratorBinding {
    IntegratorBinding::new(
        IntegratorBindingId::new("b-1").unwrap(),
        IntegratorScope::ceremony(ceremony_id()),
        RoleId::new(INTEGRATOR).unwrap(),
        HostDestination::new(
            HostKind::new("claude-code").unwrap(),
            HostAddress::new("session-1").unwrap(),
            HostActivationMode::None,
        ),
        HostAgentIncarnation::new("run-1").unwrap(),
        now(),
    )
}

fn input() -> AwaitIntegratorAttentionInput {
    AwaitIntegratorAttentionInput {
        scope: IntegratorScope::ceremony(ceremony_id()),
        binding_id: IntegratorBindingId::new("b-1").unwrap(),
        incarnation: HostAgentIncarnation::new("run-1").unwrap(),
        fence: IntegratorFence::FIRST,
        limit: HostDeliveryPageLimit::default(),
        wait: DurationMs::ZERO,
        lease_duration: DurationMs::from_millis(30_000),
    }
}

/// A ceremony with one sealed step result in its feed, and the
/// delivery an integrator would have been offered for it.
async fn seeded() -> (Arc<EventStoreFake>, Arc<LedgerFake>, AttentionEventId) {
    let store = store_with_a_sealed_step_result().await;
    let id = news_in(&store).await;
    let ledger = Arc::new(LedgerFake::default());
    ledger
        .enqueue(
            HostDeliveryRecord::queued(
                HostDeliveryItem::attention(ceremony_id(), id.clone()),
                binding().delivery_target(),
                HostDeliveryPolicy::pull(),
                now(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    (store, ledger, id)
}

/// The attention identity of the sealed result in this feed.
///
/// Seeding the session appended a record of its own, so the result
/// is not at the head of the feed: find the one the rules read as
/// news rather than assuming where it landed.
async fn news_in(store: &Arc<EventStoreFake>) -> AttentionEventId {
    let integrator = RoleId::new(INTEGRATOR).unwrap();
    store
        .read_all(
            GlobalPosition::new(1).unwrap(),
            made_core::value_objects::CeremonyEventPageLimit::new(50).unwrap(),
        )
        .await
        .unwrap()
        .into_iter()
        .find_map(|positioned| {
            crate::services::attention::attention_for(&positioned, &integrator)
                .ok()
                .flatten()
        })
        .expect("a sealed step result is news")
        .id()
        .clone()
}

fn use_case(
    store: Arc<EventStoreFake>,
    ledger: Arc<LedgerFake>,
) -> AwaitIntegratorAttentionUseCase {
    AwaitIntegratorAttentionUseCase::new(
        Arc::new(BindingsFake::holding(binding())),
        ledger,
        store.clone(),
        stream(store),
        definition_resolver(Arc::new(DefinitionRepositoryFake::new(definition()))),
        Arc::new(QuietNotifier),
        Arc::new(FixedClock::new(now())),
    )
}

#[tokio::test]
async fn a_bound_host_is_handed_the_news_and_where_it_stands() {
    let (store, ledger, id) = seeded().await;

    let batch = use_case(store, Arc::clone(&ledger))
        .execute(input())
        .await
        .unwrap();

    assert_eq!(batch.items().len(), 1);
    let item = &batch.items()[0];
    assert_eq!(item.attention().id(), &id);
    assert_eq!(item.attention().kind(), AttentionKind::ResultAvailable);
    assert_eq!(item.context().ceremony_id(), &ceremony_id());
    assert_eq!(batch.end_reason(), AttentionEndReason::Items);
}

/// The wake-up is not the authority.
///
/// Nothing ever offered this news — no subscriber ran, which is what a
/// process that was down during the append looks like afterwards — and
/// the host still collects it, because the read walks the feed first.
#[tokio::test]
async fn a_read_recovers_news_no_subscriber_ever_offered() {
    let store = store_with_a_sealed_step_result().await;
    let ledger = Arc::new(LedgerFake::default());
    let use_case =
        use_case(Arc::clone(&store), Arc::clone(&ledger)).with_recovery(attention_recovery(
            Arc::new(BindingsFake::holding(binding())),
            Arc::clone(&ledger) as Arc<dyn HostDeliveryLedgerPort>,
            Arc::clone(&store) as Arc<dyn CeremonyEventStorePort>,
            Arc::new(FixedClock::new(now())),
        ));

    let batch = use_case.execute(input()).await.unwrap();

    assert_eq!(
        batch.items().len(),
        1,
        "the journal and the cursor are the authority, not the wake-up"
    );
    assert_eq!(
        batch.items()[0].attention().kind(),
        AttentionKind::ResultAvailable
    );
}

#[tokio::test]
async fn a_host_that_was_replaced_is_refused_and_leases_nothing() {
    let (store, ledger, _) = seeded().await;
    let stale = AwaitIntegratorAttentionInput {
        incarnation: HostAgentIncarnation::new("run-0").unwrap(),
        ..input()
    };

    let refused = use_case(store, Arc::clone(&ledger)).execute(stale).await;

    assert!(matches!(
        refused,
        Err(DomainError::Conflict {
            what: "integrator_fence"
        })
    ));
    assert_eq!(
        ledger.leases(),
        0,
        "a refusal that leased something would strand it for the live host"
    );
}

#[tokio::test]
async fn an_offer_whose_news_cannot_be_derived_is_handed_back() {
    let (store, ledger, _) = seeded().await;
    ledger.clear().await;
    ledger
        .enqueue(
            HostDeliveryRecord::queued(
                HostDeliveryItem::attention(
                    ceremony_id(),
                    AttentionEventId::new("something a host made up").unwrap(),
                ),
                binding().delivery_target(),
                HostDeliveryPolicy::pull(),
                now(),
            )
            .unwrap(),
        )
        .await
        .unwrap();

    let batch = use_case(store, Arc::clone(&ledger))
        .execute(input())
        .await
        .unwrap();

    assert!(batch.items().is_empty());
    assert_eq!(
        ledger.releases(),
        1,
        "an offer nobody can read is given back, not invented into news"
    );
}

#[tokio::test]
async fn nothing_to_do_says_come_back_rather_than_stop() {
    let (store, ledger, _) = seeded().await;
    ledger.clear().await;

    let batch = use_case(store, ledger).execute(input()).await.unwrap();

    assert!(batch.items().is_empty());
    assert_eq!(batch.end_reason(), AttentionEndReason::WaitElapsed);
    assert!(!batch.is_halting());
}

struct QuietNotifier;

impl CeremonyProgressNotifierPort for QuietNotifier {
    fn subscribe(&self) -> Box<dyn CeremonyProgressSubscriptionPort> {
        Box::new(QuietSubscription)
    }
}

struct QuietSubscription;

#[async_trait]
impl CeremonyProgressSubscriptionPort for QuietSubscription {
    async fn wait(&mut self) {
        std::future::pending::<()>().await;
    }
}

struct BindingsFake {
    live: std::sync::Mutex<IntegratorBinding>,
}

impl BindingsFake {
    fn holding(live: IntegratorBinding) -> Self {
        Self {
            live: std::sync::Mutex::new(live),
        }
    }
}

#[async_trait]
impl IntegratorBindingPort for BindingsFake {
    /// Remembered, because a loop that forgot how many rounds it had
    /// been would never reach the end of them.
    async fn record_progress(
        &self,
        id: &IntegratorBindingId,
        progress: made_core::value_objects::LoopProgressMark,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        let mut live = self.live.lock().unwrap();
        if live.id() != id {
            return Ok(None);
        }
        *live = live.observing(progress);
        Ok(Some(live.clone()))
    }

    async fn bind(
        &self,
        _binding: IntegratorBinding,
        _replacement: BindReplacement,
    ) -> Result<BindOutcome, DomainError> {
        unimplemented!("awaiting does not bind")
    }

    async fn current(
        &self,
        _scope: &IntegratorScope,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        Ok(Some(self.live.lock().unwrap().clone()))
    }

    async fn revoke(
        &self,
        _id: &IntegratorBindingId,
        _now: OffsetDateTime,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        unimplemented!("awaiting does not revoke")
    }

    async fn list(
        &self,
        _scope: Option<&IntegratorScope>,
    ) -> Result<Vec<IntegratorBinding>, DomainError> {
        unimplemented!("awaiting asks for one scope")
    }
}

#[derive(Default)]
struct LedgerFake {
    held: tokio::sync::RwLock<BTreeMap<HostDeliveryId, HostDeliveryRecord>>,
    leases: Mutex<u32>,
    releases: Mutex<u32>,
}

impl LedgerFake {
    async fn clear(&self) {
        self.held.write().await.clear();
    }

    fn leases(&self) -> u32 {
        *self.leases.lock().unwrap()
    }

    fn releases(&self) -> u32 {
        *self.releases.lock().unwrap()
    }
}

#[async_trait]
impl HostDeliveryLedgerPort for LedgerFake {
    async fn enqueue(&self, record: HostDeliveryRecord) -> Result<EnqueueOutcome, DomainError> {
        self.held
            .write()
            .await
            .insert(record.id().clone(), record.clone());
        Ok(EnqueueOutcome::Enqueued(record))
    }

    async fn lease(
        &self,
        filter: &HostDeliveryFilter,
        owner: &HostAgentIncarnation,
        now: OffsetDateTime,
        duration: DurationMs,
        limit: HostDeliveryPageLimit,
    ) -> Result<Vec<LeasedDelivery>, DomainError> {
        let mut held = self.held.write().await;
        let offerable: Vec<HostDeliveryRecord> = held
            .values()
            .filter(|record| filter.admits(record) && record.is_offerable_at(now))
            .take(limit.as_usize())
            .cloned()
            .collect();
        let mut leased = Vec::with_capacity(offerable.len());
        for record in offerable {
            let lease = HostDeliveryLease::new(
                record.id().clone(),
                HostDeliveryLeaseId::new(format!("l-{}", record.id())).unwrap(),
                owner.clone(),
                now + time::Duration::milliseconds(duration.get() as i64),
            );
            let next = record.leased(lease.clone(), now);
            held.insert(next.id().clone(), next.clone());
            *self.leases.lock().unwrap() += 1;
            leased.push(LeasedDelivery::new(lease, next));
        }
        Ok(leased)
    }

    async fn acknowledge(
        &self,
        _lease: &HostDeliveryLease,
        _observation: &HostDeliveryObservation,
        _now: OffsetDateTime,
    ) -> Result<AckOutcome, DomainError> {
        unimplemented!("acknowledging is its own use case")
    }

    async fn mark_processed(
        &self,
        _delivery_id: &HostDeliveryId,
        _owner: &HostAgentIncarnation,
        _fence: Option<IntegratorFence>,
        _action: &ProcessedActionRef,
        _now: OffsetDateTime,
    ) -> Result<ProcessedOutcome, DomainError> {
        unimplemented!("processing is its own use case")
    }

    async fn record_activation(
        &self,
        _delivery_id: &HostDeliveryId,
        _outcome: &HostActivationOutcome,
        _now: OffsetDateTime,
    ) -> Result<RecordedActivation, DomainError> {
        unimplemented!("awaiting wakes nobody")
    }

    async fn mark_failed(
        &self,
        _lease: &HostDeliveryLease,
        _reason: &DeliveryFailureReason,
        _now: OffsetDateTime,
    ) -> Result<DeliveryFailureOutcome, DomainError> {
        unimplemented!("awaiting fails nothing")
    }

    async fn release(
        &self,
        lease: &HostDeliveryLease,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let mut held = self.held.write().await;
        if let Some(record) = held.get(lease.delivery_id()).cloned() {
            held.insert(record.id().clone(), record.released(now));
        }
        *self.releases.lock().unwrap() += 1;
        Ok(())
    }

    async fn abandon(
        &self,
        _delivery_id: &HostDeliveryId,
        _cause: DeliveryExpiryCause,
        _now: OffsetDateTime,
    ) -> Result<Option<HostDeliveryRecord>, DomainError> {
        unimplemented!("awaiting gives up on nothing")
    }

    async fn expire(&self, _now: OffsetDateTime) -> Result<Vec<HostDeliveryId>, DomainError> {
        Ok(Vec::new())
    }

    async fn expire_ceremony(
        &self,
        _ceremony_id: &CeremonyId,
        _cause: DeliveryExpiryCause,
        _now: OffsetDateTime,
    ) -> Result<Vec<HostDeliveryId>, DomainError> {
        unimplemented!("awaiting ends nothing")
    }

    async fn supersede(
        &self,
        _previous: &HostDeliveryTarget,
        _replacement: &HostDeliveryTarget,
        _follow: FollowReplacement,
        _now: OffsetDateTime,
    ) -> Result<SupersessionOutcome, DomainError> {
        unimplemented!("awaiting replaces nothing")
    }

    async fn get(&self, id: &HostDeliveryId) -> Result<Option<HostDeliveryRecord>, DomainError> {
        Ok(self.held.read().await.get(id).cloned())
    }

    async fn list(&self, query: &HostDeliveryQuery) -> Result<HostDeliveryPage, DomainError> {
        let held = self.held.read().await;
        Ok(HostDeliveryPage::new(
            held.values()
                .filter(|record| query.admits(record))
                .cloned()
                .collect(),
            None,
        ))
    }
}
