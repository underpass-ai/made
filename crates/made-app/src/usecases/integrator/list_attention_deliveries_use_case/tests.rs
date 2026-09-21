//! What the ledger shows an operator about the loop.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    AckOutcome, BindOutcome, BindReplacement, CeremonyEventStorePort, DeliveryFailureOutcome,
    EnqueueOutcome, HostActivationOutcome, HostDeliveryFilter, HostDeliveryLedgerPort,
    HostDeliveryPage, HostDeliveryPageLimit, HostDeliveryQuery, IntegratorBindingPort,
    LeasedDelivery, ProcessedOutcome, RecordedActivation, SupersessionOutcome,
};
use made_core::value_objects::{
    AttentionEventId, CeremonyId, CeremonyInterventionId, DeliveryExpiryCause,
    DeliveryFailureReason, DurationMs, FollowReplacement, HostActivationMode, HostAddress,
    HostAgentIncarnation, HostDeliveryId, HostDeliveryItem, HostDeliveryItemKind,
    HostDeliveryLease, HostDeliveryObservation, HostDeliveryPolicy, HostDeliveryRecord,
    HostDeliveryState, HostDeliveryTarget, HostDestination, HostKind, IntegratorBinding,
    IntegratorBindingId, IntegratorFence, IntegratorScope, ProcessedActionRef, RoleId,
};
use time::OffsetDateTime;

use super::*;
use crate::usecases::ceremony_test_support::{
    attention_recovery, ceremony_id, store_with_a_sealed_step_result, FixedClock,
};
use crate::usecases::integrator::ListAttentionDeliveriesInput;

fn now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}

fn ceremony() -> CeremonyId {
    CeremonyId::new("loop-1").unwrap()
}

fn target() -> HostDeliveryTarget {
    HostDeliveryTarget::IntegratorBinding {
        binding_id: IntegratorBindingId::new("b-1").unwrap(),
    }
}

fn attention(id: &str) -> HostDeliveryRecord {
    HostDeliveryRecord::queued(
        HostDeliveryItem::attention(ceremony(), AttentionEventId::new(id).unwrap()),
        target(),
        HostDeliveryPolicy::pull(),
        now(),
    )
    .unwrap()
}

fn intervention() -> HostDeliveryRecord {
    HostDeliveryRecord::queued(
        HostDeliveryItem::intervention(ceremony(), CeremonyInterventionId::new("i-1").unwrap()),
        HostDeliveryTarget::role(RoleId::new("REVIEWER").unwrap()),
        HostDeliveryPolicy::pull(),
        now(),
    )
    .unwrap()
}

#[tokio::test]
async fn a_supervisors_question_is_not_the_integrator_loops_business() {
    let ledger = Arc::new(LedgerFake::holding(vec![
        attention("loop-1:1:e-1:result_available"),
        intervention(),
    ]));

    let page = ListAttentionDeliveriesUseCase::new(ledger)
        .execute(ListAttentionDeliveriesInput::default())
        .await
        .unwrap();

    assert_eq!(page.records().len(), 1);
    assert_eq!(
        page.records()[0].item().kind(),
        HostDeliveryItemKind::Attention
    );
}

#[tokio::test]
async fn an_offer_that_ended_is_still_there_with_its_cause() {
    let shed = attention("loop-1:1:e-1:result_available")
        .expired(DeliveryExpiryCause::QueueOverflow, now());
    let ledger = Arc::new(LedgerFake::holding(vec![shed]));

    let page = ListAttentionDeliveriesUseCase::new(ledger)
        .execute(ListAttentionDeliveriesInput::default())
        .await
        .unwrap();

    assert!(
        matches!(
            page.records()[0].state(),
            HostDeliveryState::Expired {
                cause: DeliveryExpiryCause::QueueOverflow,
                ..
            }
        ),
        "a queue would have swallowed this; a ledger is asked precisely for it"
    );
}

fn bound() -> IntegratorBinding {
    IntegratorBinding::new(
        IntegratorBindingId::new("b-1").unwrap(),
        IntegratorScope::ceremony(ceremony_id()),
        RoleId::new("INTEGRATOR").unwrap(),
        HostDestination::new(
            HostKind::new("claude-code").unwrap(),
            HostAddress::new("session-1").unwrap(),
            HostActivationMode::None,
        ),
        HostAgentIncarnation::new("run-1").unwrap(),
        now(),
    )
}

/// An operator asking about a deployment nothing has projected for
/// reads a ledger this call filled, not the empty one it found.
#[tokio::test]
async fn the_read_projects_before_it_answers() {
    let store = store_with_a_sealed_step_result().await;
    let ledger = Arc::new(LedgerFake::default());
    let use_case =
        ListAttentionDeliveriesUseCase::new(Arc::clone(&ledger) as Arc<dyn HostDeliveryLedgerPort>)
            .with_recovery(attention_recovery(
                Arc::new(BindingsFake { live: bound() }),
                Arc::clone(&ledger) as Arc<dyn HostDeliveryLedgerPort>,
                store as Arc<dyn CeremonyEventStorePort>,
                Arc::new(FixedClock::new(now())),
            ));

    let page = use_case
        .execute(ListAttentionDeliveriesInput::default())
        .await
        .unwrap();

    assert_eq!(
        page.records().len(),
        1,
        "an empty queue on a host that is behind is the wrong answer"
    );
    assert_eq!(
        page.records()[0].item().kind(),
        HostDeliveryItemKind::Attention
    );
}

#[derive(Default)]
struct LedgerFake {
    held: Mutex<Vec<HostDeliveryRecord>>,
}

/// One live binding, which is what the projection asks the store for.
struct BindingsFake {
    live: IntegratorBinding,
}

#[async_trait]
impl IntegratorBindingPort for BindingsFake {
    async fn bind(
        &self,
        _binding: IntegratorBinding,
        _replacement: BindReplacement,
    ) -> Result<BindOutcome, DomainError> {
        unimplemented!("listing binds nothing")
    }

    async fn current(
        &self,
        _scope: &IntegratorScope,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        unimplemented!("listing does not know which scope to ask for")
    }

    async fn revoke(
        &self,
        _id: &IntegratorBindingId,
        _now: OffsetDateTime,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        unimplemented!("listing revokes nothing")
    }

    async fn list(
        &self,
        _scope: Option<&IntegratorScope>,
    ) -> Result<Vec<IntegratorBinding>, DomainError> {
        Ok(vec![self.live.clone()])
    }
}

impl LedgerFake {
    fn holding(records: Vec<HostDeliveryRecord>) -> Self {
        Self {
            held: Mutex::new(records),
        }
    }
}

#[async_trait]
impl HostDeliveryLedgerPort for LedgerFake {
    async fn enqueue(&self, record: HostDeliveryRecord) -> Result<EnqueueOutcome, DomainError> {
        let mut held = self.held.lock().expect("the ledger is writable");
        if let Some(existing) = held.iter().find(|held| held.id() == record.id()) {
            return Ok(EnqueueOutcome::AlreadyQueued(existing.clone()));
        }
        held.push(record.clone());
        Ok(EnqueueOutcome::Enqueued(record))
    }

    async fn lease(
        &self,
        _filter: &HostDeliveryFilter,
        _owner: &HostAgentIncarnation,
        _now: OffsetDateTime,
        _duration: DurationMs,
        _limit: HostDeliveryPageLimit,
    ) -> Result<Vec<LeasedDelivery>, DomainError> {
        unimplemented!("listing takes nothing")
    }

    async fn acknowledge(
        &self,
        _lease: &HostDeliveryLease,
        _observation: &HostDeliveryObservation,
        _now: OffsetDateTime,
    ) -> Result<AckOutcome, DomainError> {
        unimplemented!("listing answers nothing")
    }

    async fn mark_processed(
        &self,
        _delivery_id: &HostDeliveryId,
        _owner: &HostAgentIncarnation,
        _fence: Option<IntegratorFence>,
        _action: &ProcessedActionRef,
        _now: OffsetDateTime,
    ) -> Result<ProcessedOutcome, DomainError> {
        unimplemented!("listing closes nothing")
    }

    async fn record_activation(
        &self,
        _delivery_id: &HostDeliveryId,
        _outcome: &HostActivationOutcome,
        _now: OffsetDateTime,
    ) -> Result<RecordedActivation, DomainError> {
        unimplemented!("listing wakes nobody")
    }

    async fn mark_failed(
        &self,
        _lease: &HostDeliveryLease,
        _reason: &DeliveryFailureReason,
        _now: OffsetDateTime,
    ) -> Result<DeliveryFailureOutcome, DomainError> {
        unimplemented!("listing fails nothing")
    }

    async fn release(
        &self,
        _lease: &HostDeliveryLease,
        _now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        unimplemented!("listing holds nothing")
    }

    async fn abandon(
        &self,
        _delivery_id: &HostDeliveryId,
        _cause: DeliveryExpiryCause,
        _now: OffsetDateTime,
    ) -> Result<Option<HostDeliveryRecord>, DomainError> {
        unimplemented!("listing gives up on nothing")
    }

    async fn expire(&self, _now: OffsetDateTime) -> Result<Vec<HostDeliveryId>, DomainError> {
        unimplemented!("listing sweeps nothing")
    }

    async fn expire_ceremony(
        &self,
        _ceremony_id: &CeremonyId,
        _cause: DeliveryExpiryCause,
        _now: OffsetDateTime,
    ) -> Result<Vec<HostDeliveryId>, DomainError> {
        unimplemented!("listing ends nothing")
    }

    async fn supersede(
        &self,
        _previous: &HostDeliveryTarget,
        _replacement: &HostDeliveryTarget,
        _follow: FollowReplacement,
        _now: OffsetDateTime,
    ) -> Result<SupersessionOutcome, DomainError> {
        unimplemented!("listing replaces nothing")
    }

    async fn get(&self, _id: &HostDeliveryId) -> Result<Option<HostDeliveryRecord>, DomainError> {
        unimplemented!("listing asks for a page")
    }

    async fn list(&self, query: &HostDeliveryQuery) -> Result<HostDeliveryPage, DomainError> {
        let held = self.held.lock().expect("the ledger is readable");
        Ok(HostDeliveryPage::new(
            held.iter()
                .filter(|record| query.admits(record))
                .cloned()
                .collect(),
            None,
        ))
    }
}
