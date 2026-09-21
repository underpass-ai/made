//! Saying what you are about to do, and later that you did it.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    AckOutcome, BindOutcome, BindReplacement, DeliveryFailureOutcome, EnqueueOutcome,
    HostActivationOutcome, HostDeliveryFilter, HostDeliveryLedgerPort, HostDeliveryPage,
    HostDeliveryPageLimit, HostDeliveryQuery, IntegratorBindingPort, LeasedDelivery,
    ProcessedOutcome, RecordedActivation, SupersessionOutcome,
};
use made_core::value_objects::{
    AttentionEventId, CeremonyId, DeliveryExpiryCause, DeliveryFailureReason, DeliveryNote,
    DurationMs, FollowReplacement, HostActivationMode, HostAddress, HostAgentIncarnation,
    HostDeliveryId, HostDeliveryItem, HostDeliveryLease, HostDeliveryLeaseId,
    HostDeliveryObservation, HostDeliveryPolicy, HostDeliveryRecord, HostDeliveryStateKind,
    HostDeliveryTarget, HostDestination, HostKind, IntegratorBinding, IntegratorBindingId,
    IntegratorFence, IntegratorScope, ProcessedActionKind, ProcessedActionRef, RoleId,
};
use tokio::sync::RwLock;

use super::*;
use crate::usecases::integrator::{AcknowledgeIntegratorAttentionInput, IntegratorAcknowledgement};

fn now() -> time::OffsetDateTime {
    time::OffsetDateTime::UNIX_EPOCH
}

fn ceremony() -> CeremonyId {
    CeremonyId::new("loop-1").unwrap()
}

fn incarnation() -> HostAgentIncarnation {
    HostAgentIncarnation::new("run-1").unwrap()
}

fn binding() -> IntegratorBinding {
    IntegratorBinding::new(
        IntegratorBindingId::new("b-1").unwrap(),
        IntegratorScope::ceremony(ceremony()),
        RoleId::new("INTEGRATOR").unwrap(),
        HostDestination::new(
            HostKind::new("claude-code").unwrap(),
            HostAddress::new("session-1").unwrap(),
            HostActivationMode::None,
        ),
        incarnation(),
        now(),
    )
}

fn offered() -> HostDeliveryRecord {
    HostDeliveryRecord::queued(
        HostDeliveryItem::attention(
            ceremony(),
            AttentionEventId::new("loop-1:1:e-1:result_available").unwrap(),
        ),
        binding().delivery_target(),
        HostDeliveryPolicy::pull(),
        now(),
    )
    .unwrap()
}

fn lease_of(record: &HostDeliveryRecord) -> HostDeliveryLease {
    HostDeliveryLease::new(
        record.id().clone(),
        HostDeliveryLeaseId::new("l-1").unwrap(),
        incarnation(),
        now() + time::Duration::seconds(30),
    )
}

fn input(
    record: &HostDeliveryRecord,
    outcome: IntegratorAcknowledgement,
) -> AcknowledgeIntegratorAttentionInput {
    AcknowledgeIntegratorAttentionInput {
        binding_id: IntegratorBindingId::new("b-1").unwrap(),
        incarnation: incarnation(),
        fence: IntegratorFence::FIRST,
        delivery_id: record.id().clone(),
        lease: lease_of(record),
        outcome,
    }
}

fn intent() -> IntegratorAcknowledgement {
    IntegratorAcknowledgement::Intent {
        action: ProcessedActionRef::of(ProcessedActionKind::Delegated),
        note: DeliveryNote::new("handing the result to the implementer").unwrap(),
        evidence: None,
    }
}

async fn leased_ledger() -> (Arc<LedgerFake>, HostDeliveryRecord) {
    let record = offered();
    let ledger = Arc::new(LedgerFake::default());
    let held = record.leased(lease_of(&record), now());
    ledger.held.write().await.insert(held.id().clone(), held);
    (ledger, record)
}

fn use_case(ledger: Arc<LedgerFake>) -> AcknowledgeIntegratorAttentionUseCase {
    AcknowledgeIntegratorAttentionUseCase::new(
        Arc::new(BindingsFake),
        ledger,
        Arc::new(FrozenClock),
    )
}

#[tokio::test]
async fn an_intent_goes_on_the_record_and_the_host_keeps_holding_the_work() {
    let (ledger, record) = leased_ledger().await;

    let answered = use_case(Arc::clone(&ledger))
        .execute(input(&record, intent()))
        .await
        .unwrap();

    assert!(matches!(
        answered,
        IntegratorAttentionAcknowledged::Intent { .. }
    ));
    assert_eq!(
        ledger.state_of(record.id()).await,
        Some(HostDeliveryStateKind::Acknowledged),
        "an intent is received, not processed: the host has not acted yet"
    );
}

#[tokio::test]
async fn saying_it_acted_closes_the_delivery() {
    let (ledger, record) = leased_ledger().await;
    let case = use_case(Arc::clone(&ledger));
    case.execute(input(&record, intent())).await.unwrap();

    let answered = case
        .execute(input(
            &record,
            IntegratorAcknowledgement::Processed {
                action: ProcessedActionRef::of(ProcessedActionKind::Delegated),
            },
        ))
        .await
        .unwrap();

    assert!(matches!(
        answered,
        IntegratorAttentionAcknowledged::Processed(ProcessedOutcome::Processed(_))
    ));
    assert_eq!(
        ledger.state_of(record.id()).await,
        Some(HostDeliveryStateKind::Processed)
    );
}

#[tokio::test]
async fn claiming_to_have_acted_on_something_never_received_is_refused() {
    let (ledger, record) = leased_ledger().await;

    let answered = use_case(Arc::clone(&ledger))
        .execute(input(
            &record,
            IntegratorAcknowledgement::Processed {
                action: ProcessedActionRef::of(ProcessedActionKind::Delegated),
            },
        ))
        .await
        .unwrap();

    assert!(
        matches!(
            answered,
            IntegratorAttentionAcknowledged::Processed(ProcessedOutcome::NotAcknowledged { .. })
        ),
        "closing a hand-off nobody made would lose track of which delivery was answered"
    );
}

#[tokio::test]
async fn a_host_that_was_replaced_cannot_close_its_successors_work() {
    let (ledger, record) = leased_ledger().await;
    let stale = AcknowledgeIntegratorAttentionInput {
        incarnation: HostAgentIncarnation::new("run-0").unwrap(),
        ..input(&record, intent())
    };

    let refused = use_case(Arc::clone(&ledger)).execute(stale).await;

    assert!(matches!(
        refused,
        Err(DomainError::Conflict {
            what: "integrator_fence"
        })
    ));
    assert_eq!(
        ledger.state_of(record.id()).await,
        Some(HostDeliveryStateKind::Leased),
        "a refusal must not move the delivery the live host still holds"
    );
}

struct FrozenClock;

impl made_core::ports::ClockPort for FrozenClock {
    fn now(&self) -> time::OffsetDateTime {
        now()
    }
}

struct BindingsFake;

#[async_trait]
impl IntegratorBindingPort for BindingsFake {
    async fn bind(
        &self,
        _binding: IntegratorBinding,
        _replacement: BindReplacement,
    ) -> Result<BindOutcome, DomainError> {
        unimplemented!("acknowledging does not bind")
    }

    async fn current(
        &self,
        _scope: &IntegratorScope,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        unimplemented!("acknowledging looks up by binding, not by scope")
    }

    async fn revoke(
        &self,
        _id: &IntegratorBindingId,
        _now: time::OffsetDateTime,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        unimplemented!("acknowledging does not revoke")
    }

    async fn list(
        &self,
        _scope: Option<&IntegratorScope>,
    ) -> Result<Vec<IntegratorBinding>, DomainError> {
        Ok(vec![binding()])
    }
}

#[derive(Default)]
struct LedgerFake {
    held: RwLock<BTreeMap<HostDeliveryId, HostDeliveryRecord>>,
}

impl LedgerFake {
    async fn state_of(&self, id: &HostDeliveryId) -> Option<HostDeliveryStateKind> {
        self.held.read().await.get(id).map(|r| r.state().kind())
    }
}

#[async_trait]
impl HostDeliveryLedgerPort for LedgerFake {
    async fn enqueue(&self, _record: HostDeliveryRecord) -> Result<EnqueueOutcome, DomainError> {
        unimplemented!("acknowledging offers nothing")
    }

    async fn lease(
        &self,
        _filter: &HostDeliveryFilter,
        _owner: &HostAgentIncarnation,
        _now: time::OffsetDateTime,
        _duration: DurationMs,
        _limit: HostDeliveryPageLimit,
    ) -> Result<Vec<LeasedDelivery>, DomainError> {
        unimplemented!("acknowledging takes nothing")
    }

    async fn acknowledge(
        &self,
        lease: &HostDeliveryLease,
        observation: &HostDeliveryObservation,
        now: time::OffsetDateTime,
    ) -> Result<AckOutcome, DomainError> {
        let mut held = self.held.write().await;
        let Some(record) = held.get(lease.delivery_id()).cloned() else {
            return Ok(AckOutcome::LeaseNotOwned);
        };
        if record.live_lease_at(now).map(HostDeliveryLease::lease_id) != Some(lease.lease_id()) {
            return Ok(AckOutcome::LeaseNotOwned);
        }
        let next = record.acknowledged(observation.clone(), now);
        held.insert(next.id().clone(), next.clone());
        Ok(AckOutcome::Acknowledged(next))
    }

    async fn mark_processed(
        &self,
        delivery_id: &HostDeliveryId,
        _owner: &HostAgentIncarnation,
        _fence: Option<IntegratorFence>,
        action: &ProcessedActionRef,
        now: time::OffsetDateTime,
    ) -> Result<ProcessedOutcome, DomainError> {
        let mut held = self.held.write().await;
        let Some(record) = held.get(delivery_id).cloned() else {
            return Ok(ProcessedOutcome::LeaseNotOwned);
        };
        let state = record.state().kind();
        if state != HostDeliveryStateKind::Acknowledged {
            return Ok(ProcessedOutcome::NotAcknowledged { state });
        }
        let next = record.processed(action.clone(), now);
        held.insert(next.id().clone(), next.clone());
        Ok(ProcessedOutcome::Processed(next))
    }

    async fn record_activation(
        &self,
        _delivery_id: &HostDeliveryId,
        _outcome: &HostActivationOutcome,
        _now: time::OffsetDateTime,
    ) -> Result<RecordedActivation, DomainError> {
        unimplemented!("acknowledging wakes nobody")
    }

    async fn mark_failed(
        &self,
        lease: &HostDeliveryLease,
        reason: &DeliveryFailureReason,
        now: time::OffsetDateTime,
    ) -> Result<DeliveryFailureOutcome, DomainError> {
        let mut held = self.held.write().await;
        let Some(record) = held.get(lease.delivery_id()).cloned() else {
            return Ok(DeliveryFailureOutcome::LeaseNotOwned);
        };
        let next = record.failed(reason.clone(), now);
        held.insert(next.id().clone(), next.clone());
        Ok(DeliveryFailureOutcome::Exhausted(next))
    }

    async fn release(
        &self,
        _lease: &HostDeliveryLease,
        _now: time::OffsetDateTime,
    ) -> Result<(), DomainError> {
        unimplemented!("acknowledging does not hand back")
    }

    async fn abandon(
        &self,
        _delivery_id: &HostDeliveryId,
        _cause: DeliveryExpiryCause,
        _now: time::OffsetDateTime,
    ) -> Result<Option<HostDeliveryRecord>, DomainError> {
        unimplemented!("acknowledging gives up on nothing")
    }

    async fn expire(&self, _now: time::OffsetDateTime) -> Result<Vec<HostDeliveryId>, DomainError> {
        unimplemented!("acknowledging sweeps nothing")
    }

    async fn expire_ceremony(
        &self,
        _ceremony_id: &CeremonyId,
        _cause: DeliveryExpiryCause,
        _now: time::OffsetDateTime,
    ) -> Result<Vec<HostDeliveryId>, DomainError> {
        unimplemented!("acknowledging ends nothing")
    }

    async fn supersede(
        &self,
        _previous: &HostDeliveryTarget,
        _replacement: &HostDeliveryTarget,
        _follow: FollowReplacement,
        _now: time::OffsetDateTime,
    ) -> Result<SupersessionOutcome, DomainError> {
        unimplemented!("acknowledging replaces nothing")
    }

    async fn get(&self, id: &HostDeliveryId) -> Result<Option<HostDeliveryRecord>, DomainError> {
        Ok(self.held.read().await.get(id).cloned())
    }

    async fn list(&self, _query: &HostDeliveryQuery) -> Result<HostDeliveryPage, DomainError> {
        unimplemented!("acknowledging lists nothing")
    }
}
