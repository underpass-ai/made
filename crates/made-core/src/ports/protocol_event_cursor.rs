//! Durable named progress over the global protocol-event order.

use async_trait::async_trait;
use time::OffsetDateTime;

use crate::error::DomainError;
use crate::value_objects::{
    DurationMs, GlobalPosition, ProtocolEventConsumer, ProtocolEventCursorLease,
    ProtocolEventCursorLeaseId, ProtocolEventQuarantineReason, QuarantinedProtocolEvent,
};

/// Persistence contract for independent consumers of the global feed.
#[async_trait]
pub trait ProtocolEventCursorPort: Send + Sync {
    /// Last position explicitly acknowledged by the consumer.
    async fn position(
        &self,
        consumer: &ProtocolEventConsumer,
    ) -> Result<Option<GlobalPosition>, DomainError>;

    /// Acquire exclusive, expiring processing rights for one consumer.
    async fn lease(
        &self,
        consumer: &ProtocolEventConsumer,
        lease_id: ProtocolEventCursorLeaseId,
        now: OffsetDateTime,
        duration: DurationMs,
    ) -> Result<Option<ProtocolEventCursorLease>, DomainError>;

    /// Advance monotonically after an explicit pull acknowledgement.
    ///
    /// A stale acknowledgement is a no-op. A new acknowledgement is refused
    /// while a publisher holds a lease, so an out-of-band pull cannot clear or
    /// advance work that is currently fenced to another worker.
    async fn acknowledge(
        &self,
        consumer: &ProtocolEventConsumer,
        through: GlobalPosition,
    ) -> Result<(), DomainError>;

    /// Advance after delivery by the worker that owns the current lease.
    ///
    /// The lease id is the fencing token: a worker whose lease was replaced
    /// cannot commit delivery or disturb the replacement lease.
    async fn acknowledge_lease(
        &self,
        lease: &ProtocolEventCursorLease,
        through: GlobalPosition,
    ) -> Result<(), DomainError>;

    /// Record a failed delivery at this position and release the lease.
    async fn mark_failed(
        &self,
        lease: &ProtocolEventCursorLease,
        position: GlobalPosition,
    ) -> Result<(), DomainError>;

    /// Make an exhausted failure visible, advance past it, and release the lease.
    async fn quarantine(
        &self,
        lease: &ProtocolEventCursorLease,
        position: GlobalPosition,
        reason: ProtocolEventQuarantineReason,
        now: OffsetDateTime,
    ) -> Result<(), DomainError>;

    /// Give up an idle lease without changing progress.
    async fn release(&self, lease: &ProtocolEventCursorLease) -> Result<(), DomainError>;

    /// Every event this consumer deliberately skipped, in global order.
    async fn quarantined(
        &self,
        consumer: &ProtocolEventConsumer,
    ) -> Result<Vec<QuarantinedProtocolEvent>, DomainError>;
}
