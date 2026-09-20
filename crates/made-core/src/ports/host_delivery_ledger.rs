//! Durable delivery of one item to one host destination.

use async_trait::async_trait;
use time::OffsetDateTime;

use crate::error::DomainError;
use crate::value_objects::{
    DeliveryFailureReason, DurationMs, FollowReplacement, HostAgentIncarnation, HostDeliveryId,
    HostDeliveryLease, HostDeliveryObservation, HostDeliveryRecord, HostDeliveryTarget,
    IntegratorFence, ProcessedActionRef,
};

use super::{
    AckOutcome, DeliveryFailureOutcome, EnqueueOutcome, HostDeliveryFilter, HostDeliveryPage,
    HostDeliveryPageLimit, HostDeliveryQuery, LeasedDelivery, ProcessedOutcome,
    SupersessionOutcome,
};

/// Persistence contract for work handed out to hosts.
///
/// Modelled on the named event cursor rather than on a queue: an
/// exclusive expiring lease, a counted attempt, and an end that stays
/// visible. A queue whose acknowledgement consumes the item cannot
/// answer the question an operator actually asks, which is what
/// happened to the question nobody ever came back about.
#[async_trait]
pub trait HostDeliveryLedgerPort: Send + Sync {
    /// Offer a delivery, or recognise the one already held.
    async fn enqueue(&self, record: HostDeliveryRecord) -> Result<EnqueueOutcome, DomainError>;

    /// Hand out, exclusively and with an expiry, what a host can take.
    ///
    /// A delivery whose lease has expired is offerable again: the lease
    /// bounds the exclusion, so a host that died holding one strands
    /// nothing.
    async fn lease(
        &self,
        filter: &HostDeliveryFilter,
        owner: &HostAgentIncarnation,
        now: OffsetDateTime,
        duration: DurationMs,
        limit: HostDeliveryPageLimit,
    ) -> Result<Vec<LeasedDelivery>, DomainError>;

    /// Record what the host said about a delivery it holds.
    async fn acknowledge(
        &self,
        lease: &HostDeliveryLease,
        observation: &HostDeliveryObservation,
        now: OffsetDateTime,
    ) -> Result<AckOutcome, DomainError>;

    /// Close a delivery the host has acted on.
    ///
    /// Only from acknowledged, because acting on something never
    /// received is not a thing that can have happened. The fence, when
    /// the caller has one, is checked before anything is written.
    async fn mark_processed(
        &self,
        delivery_id: &HostDeliveryId,
        owner: &HostAgentIncarnation,
        fence: Option<IntegratorFence>,
        action: &ProcessedActionRef,
        now: OffsetDateTime,
    ) -> Result<ProcessedOutcome, DomainError>;

    /// Count a failed attempt, and retry or give up by the policy.
    async fn mark_failed(
        &self,
        lease: &HostDeliveryLease,
        reason: &DeliveryFailureReason,
        now: OffsetDateTime,
    ) -> Result<DeliveryFailureOutcome, DomainError>;

    /// Give a lease back without counting an attempt against it.
    async fn release(
        &self,
        lease: &HostDeliveryLease,
        now: OffsetDateTime,
    ) -> Result<(), DomainError>;

    /// Expire what has run out: leases, and acknowledgements nobody closed.
    async fn expire(&self, now: OffsetDateTime) -> Result<Vec<HostDeliveryId>, DomainError>;

    /// Follow a destination that was replaced, or leave its work behind.
    async fn supersede(
        &self,
        previous: &HostDeliveryTarget,
        replacement: &HostDeliveryTarget,
        follow: FollowReplacement,
        now: OffsetDateTime,
    ) -> Result<SupersessionOutcome, DomainError>;

    /// One delivery, whatever state it is in.
    async fn get(&self, id: &HostDeliveryId) -> Result<Option<HostDeliveryRecord>, DomainError>;

    /// One page of what the ledger holds.
    async fn list(&self, query: &HostDeliveryQuery) -> Result<HostDeliveryPage, DomainError>;
}
