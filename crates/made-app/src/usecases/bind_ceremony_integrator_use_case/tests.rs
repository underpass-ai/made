//! Binding an integrator, and what happens to the work the last one
//! never answered.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    AckOutcome, BindOutcome, BindReplacement, ClockPort, DeliveryFailureOutcome, EnqueueOutcome,
    HostActivationOutcome, HostDeliveryFilter, HostDeliveryLedgerPort, HostDeliveryPage,
    HostDeliveryPageLimit, HostDeliveryQuery, IntegratorBindingPort, LeasedDelivery,
    ProcessedOutcome, RecordedActivation, SupersessionOutcome,
};
use made_core::value_objects::{
    AttentionEventId, CeremonyId, DeliveryExpiryCause, DeliveryFailureReason, DurationMs,
    FollowReplacement, HostActivationMode, HostAddress, HostAgentIncarnation, HostDeliveryId,
    HostDeliveryItem, HostDeliveryLease, HostDeliveryObservation, HostDeliveryPolicy,
    HostDeliveryRecord, HostDeliveryTarget, HostDestination, HostKind, IntegratorBinding,
    IntegratorBindingId, IntegratorFence, IntegratorScope, ProcessedActionRef, RoleId,
};
use time::OffsetDateTime;

use super::*;
use crate::usecases::BindCeremonyIntegratorInput;

fn now() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}

fn destination() -> HostDestination {
    HostDestination::new(
        HostKind::new("claude-code").unwrap(),
        HostAddress::new("session-1").unwrap(),
        HostActivationMode::None,
    )
}

fn input(
    id: &str,
    replacement: BindReplacement,
    follow: FollowReplacement,
) -> BindCeremonyIntegratorInput {
    BindCeremonyIntegratorInput {
        binding_id: IntegratorBindingId::new(id).unwrap(),
        scope: IntegratorScope::ceremony(CeremonyId::new("loop-1").unwrap()),
        role_id: RoleId::new("INTEGRATOR").unwrap(),
        destination: destination(),
        incarnation: HostAgentIncarnation::new(format!("run-{id}")).unwrap(),
        replacement,
        follow,
    }
}

fn use_case(bindings: Arc<BindingsFake>, ledger: Arc<LedgerFake>) -> BindCeremonyIntegratorUseCase {
    BindCeremonyIntegratorUseCase::new(bindings, ledger, Arc::new(FrozenClock))
}

#[tokio::test]
async fn binding_a_scope_nobody_drives_puts_this_host_in_charge() {
    let bindings = Arc::new(BindingsFake::default());
    let ledger = Arc::new(LedgerFake::default());

    let outcome = use_case(Arc::clone(&bindings), Arc::clone(&ledger))
        .execute(input(
            "b-1",
            BindReplacement::Refuse,
            FollowReplacement::Follow,
        ))
        .await
        .unwrap();

    assert!(matches!(outcome, BindOutcome::Bound(_)));
    assert_eq!(ledger.supersessions(), 0, "nothing was owed to anybody yet");
}

#[tokio::test]
async fn replacing_a_host_moves_what_the_last_one_never_answered() {
    let bindings = Arc::new(BindingsFake::default());
    let ledger = Arc::new(LedgerFake::default());
    let case = use_case(Arc::clone(&bindings), Arc::clone(&ledger));
    case.execute(input(
        "b-1",
        BindReplacement::Refuse,
        FollowReplacement::Follow,
    ))
    .await
    .unwrap();

    let outcome = case
        .execute(input(
            "b-2",
            BindReplacement::Replace,
            FollowReplacement::Follow,
        ))
        .await
        .unwrap();

    assert!(matches!(outcome, BindOutcome::Replaced { .. }));
    assert_eq!(
        ledger.supersessions(),
        1,
        "work addressed to a host nobody is listening to is the failure this prevents"
    );
    assert_eq!(ledger.last_follow(), Some(FollowReplacement::Follow));
}

#[tokio::test]
async fn a_scope_that_is_taken_is_not_quietly_handed_over() {
    let bindings = Arc::new(BindingsFake::default());
    let ledger = Arc::new(LedgerFake::default());
    let case = use_case(Arc::clone(&bindings), Arc::clone(&ledger));
    case.execute(input(
        "b-1",
        BindReplacement::Refuse,
        FollowReplacement::Follow,
    ))
    .await
    .unwrap();

    let outcome = case
        .execute(input(
            "b-2",
            BindReplacement::Refuse,
            FollowReplacement::Follow,
        ))
        .await
        .unwrap();

    assert!(matches!(outcome, BindOutcome::AlreadyExists { .. }));
    assert_eq!(ledger.supersessions(), 0);
}

#[tokio::test]
async fn the_binding_stands_even_when_the_work_cannot_be_moved() {
    let bindings = Arc::new(BindingsFake::default());
    let ledger = Arc::new(LedgerFake::refusing());
    let case = use_case(Arc::clone(&bindings), Arc::clone(&ledger));
    case.execute(input(
        "b-1",
        BindReplacement::Refuse,
        FollowReplacement::Follow,
    ))
    .await
    .unwrap();

    let outcome = case
        .execute(input(
            "b-2",
            BindReplacement::Replace,
            FollowReplacement::Follow,
        ))
        .await
        .unwrap();

    assert!(
        matches!(outcome, BindOutcome::Replaced { .. }),
        "the fence is already up; telling the caller it is not bound would be a lie"
    );
}

struct FrozenClock;

impl ClockPort for FrozenClock {
    fn now(&self) -> OffsetDateTime {
        now()
    }
}

#[derive(Default)]
struct BindingsFake {
    live: Mutex<BTreeMap<String, IntegratorBinding>>,
}

#[async_trait]
impl IntegratorBindingPort for BindingsFake {
    async fn bind(
        &self,
        binding: IntegratorBinding,
        replacement: BindReplacement,
    ) -> Result<BindOutcome, DomainError> {
        let mut live = self.live.lock().unwrap();
        let key = binding.scope_key().to_string();
        match live.get(&key).cloned() {
            None => {
                live.insert(key, binding.clone());
                Ok(BindOutcome::Bound(binding))
            }
            Some(existing) if existing.id() == binding.id() => {
                Ok(BindOutcome::AlreadyBound(existing))
            }
            Some(existing) if replacement.replaces() => {
                let current = existing.replacing(&binding);
                live.insert(key, binding.clone());
                Ok(BindOutcome::Replaced {
                    previous: Box::new(current),
                    current: Box::new(binding),
                })
            }
            Some(existing) => Ok(BindOutcome::AlreadyExists {
                existing: Box::new(existing),
            }),
        }
    }

    async fn current(
        &self,
        scope: &IntegratorScope,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        Ok(self
            .live
            .lock()
            .unwrap()
            .get(&scope.scope_key().to_string())
            .cloned())
    }

    async fn revoke(
        &self,
        _id: &IntegratorBindingId,
        _now: OffsetDateTime,
    ) -> Result<Option<IntegratorBinding>, DomainError> {
        unimplemented!("binding does not revoke")
    }

    async fn list(
        &self,
        _scope: Option<&IntegratorScope>,
    ) -> Result<Vec<IntegratorBinding>, DomainError> {
        unimplemented!("binding does not enumerate")
    }
}

#[derive(Default)]
struct LedgerFake {
    supersessions: Mutex<u32>,
    last_follow: Mutex<Option<FollowReplacement>>,
    refuse: bool,
}

impl LedgerFake {
    fn refusing() -> Self {
        Self {
            refuse: true,
            ..Self::default()
        }
    }

    fn supersessions(&self) -> u32 {
        *self.supersessions.lock().unwrap()
    }

    fn last_follow(&self) -> Option<FollowReplacement> {
        *self.last_follow.lock().unwrap()
    }
}

#[async_trait]
impl HostDeliveryLedgerPort for LedgerFake {
    async fn enqueue(&self, _record: HostDeliveryRecord) -> Result<EnqueueOutcome, DomainError> {
        unimplemented!("binding offers nothing")
    }

    async fn lease(
        &self,
        _filter: &HostDeliveryFilter,
        _owner: &HostAgentIncarnation,
        _now: OffsetDateTime,
        _duration: DurationMs,
        _limit: HostDeliveryPageLimit,
    ) -> Result<Vec<LeasedDelivery>, DomainError> {
        unimplemented!("binding takes nothing")
    }

    async fn acknowledge(
        &self,
        _lease: &HostDeliveryLease,
        _observation: &HostDeliveryObservation,
        _now: OffsetDateTime,
    ) -> Result<AckOutcome, DomainError> {
        unimplemented!("binding answers nothing")
    }

    async fn mark_processed(
        &self,
        _delivery_id: &HostDeliveryId,
        _owner: &HostAgentIncarnation,
        _fence: Option<IntegratorFence>,
        _action: &ProcessedActionRef,
        _now: OffsetDateTime,
    ) -> Result<ProcessedOutcome, DomainError> {
        unimplemented!("binding closes nothing")
    }

    async fn record_activation(
        &self,
        _delivery_id: &HostDeliveryId,
        _outcome: &HostActivationOutcome,
        _now: OffsetDateTime,
    ) -> Result<RecordedActivation, DomainError> {
        unimplemented!("binding wakes nobody")
    }

    async fn mark_failed(
        &self,
        _lease: &HostDeliveryLease,
        _reason: &DeliveryFailureReason,
        _now: OffsetDateTime,
    ) -> Result<DeliveryFailureOutcome, DomainError> {
        unimplemented!("binding fails nothing")
    }

    async fn release(
        &self,
        _lease: &HostDeliveryLease,
        _now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        unimplemented!("binding holds nothing")
    }

    async fn abandon(
        &self,
        _delivery_id: &HostDeliveryId,
        _cause: DeliveryExpiryCause,
        _now: OffsetDateTime,
    ) -> Result<Option<HostDeliveryRecord>, DomainError> {
        unimplemented!("binding gives up on nothing")
    }

    async fn expire(&self, _now: OffsetDateTime) -> Result<Vec<HostDeliveryId>, DomainError> {
        unimplemented!("binding sweeps nothing")
    }

    async fn expire_ceremony(
        &self,
        _ceremony_id: &CeremonyId,
        _cause: DeliveryExpiryCause,
        _now: OffsetDateTime,
    ) -> Result<Vec<HostDeliveryId>, DomainError> {
        unimplemented!("binding ends nothing")
    }

    async fn supersede(
        &self,
        _previous: &HostDeliveryTarget,
        replacement: &HostDeliveryTarget,
        follow: FollowReplacement,
        now: OffsetDateTime,
    ) -> Result<SupersessionOutcome, DomainError> {
        if self.refuse {
            return Err(DomainError::InvariantViolated {
                reason: "the ledger is having a bad day",
            });
        }
        *self.supersessions.lock().unwrap() += 1;
        *self.last_follow.lock().unwrap() = Some(follow);
        let moved = HostDeliveryRecord::queued(
            HostDeliveryItem::attention(
                CeremonyId::new("loop-1").unwrap(),
                AttentionEventId::new("loop-1:1:e-1:result_available").unwrap(),
            ),
            replacement.clone(),
            HostDeliveryPolicy::pull(),
            now,
        )?;
        Ok(SupersessionOutcome::new(Vec::new(), vec![moved]))
    }

    async fn get(&self, _id: &HostDeliveryId) -> Result<Option<HostDeliveryRecord>, DomainError> {
        unimplemented!("binding reads nothing")
    }

    async fn list(&self, _query: &HostDeliveryQuery) -> Result<HostDeliveryPage, DomainError> {
        unimplemented!("binding lists nothing")
    }
}
