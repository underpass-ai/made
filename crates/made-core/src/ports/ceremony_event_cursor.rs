//! Durable named progress over the global ceremony-event order.

use async_trait::async_trait;
use time::OffsetDateTime;

use crate::error::DomainError;
use crate::value_objects::{
    CeremonyEventConsumer, CeremonyEventCursorLease, CeremonyEventCursorLeaseId,
    CeremonyEventQuarantineReason, DurationMs, GlobalPosition, QuarantinedCeremonyEvent,
};

/// Persistence contract for independent consumers of the global feed.
#[async_trait]
pub trait CeremonyEventCursorPort: Send + Sync {
    /// Last position explicitly acknowledged by the consumer.
    async fn position(
        &self,
        consumer: &CeremonyEventConsumer,
    ) -> Result<Option<GlobalPosition>, DomainError>;

    /// Acquire exclusive, expiring processing rights for one consumer.
    async fn lease(
        &self,
        consumer: &CeremonyEventConsumer,
        lease_id: CeremonyEventCursorLeaseId,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<CeremonyEventCursorLease>, DomainError>;

    /// Advance monotonically after an explicit pull acknowledgement.
    ///
    /// A stale acknowledgement is a no-op. A new acknowledgement is refused
    /// while a publisher holds a lease, so an out-of-band pull cannot clear or
    /// advance work that is currently fenced to another worker.
    async fn acknowledge(
        &self,
        consumer: &CeremonyEventConsumer,
        through: GlobalPosition,
    ) -> Result<(), DomainError>;

    /// Advance after delivery by the worker that owns the current lease.
    ///
    /// The lease id is the fencing token: a worker whose lease was replaced
    /// cannot commit delivery or disturb the replacement lease.
    async fn acknowledge_lease(
        &self,
        lease: &CeremonyEventCursorLease,
        through: GlobalPosition,
    ) -> Result<(), DomainError>;

    /// Record a failed delivery at this position and release the lease.
    async fn mark_failed(
        &self,
        lease: &CeremonyEventCursorLease,
        position: GlobalPosition,
    ) -> Result<(), DomainError>;

    /// Make an exhausted failure visible, advance past it, and release the lease.
    async fn quarantine(
        &self,
        lease: &CeremonyEventCursorLease,
        position: GlobalPosition,
        reason: CeremonyEventQuarantineReason,
        now: OffsetDateTime,
    ) -> Result<(), DomainError>;

    /// Give up an idle lease without changing progress.
    async fn release(&self, lease: &CeremonyEventCursorLease) -> Result<(), DomainError>;

    /// Every event this consumer deliberately skipped, in global order.
    async fn quarantined(
        &self,
        consumer: &CeremonyEventConsumer,
    ) -> Result<Vec<QuarantinedCeremonyEvent>, DomainError>;
}
