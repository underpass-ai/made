//! [`MessagingPort`] — asynchronous message bus used to publish and
//! consume domain events.
//!
//! The port speaks in domain-event terms; adapters map to/from NATS,
//! Kafka, or any other substrate without leaking transport details
//! into the core.

use crate::error::DomainError;
use crate::events::{
    DeliberationCompletedEvent, PhaseChangedEvent, TaskCompletedEvent, TaskDispatchedEvent,
    TaskFailedEvent,
};
use async_trait::async_trait;

/// Publish / subscribe surface. Intentionally narrow: specific
/// `publish_*` methods per event type enforce that publishing is a
/// first-class, audited action — publishing an arbitrary untyped
/// payload is not possible.
#[async_trait]
pub trait MessagingPort: Send + Sync {
    async fn publish_task_dispatched(&self, event: &TaskDispatchedEvent)
        -> Result<(), DomainError>;
    async fn publish_task_completed(&self, event: &TaskCompletedEvent) -> Result<(), DomainError>;
    async fn publish_task_failed(&self, event: &TaskFailedEvent) -> Result<(), DomainError>;
    async fn publish_deliberation_completed(
        &self,
        event: &DeliberationCompletedEvent,
    ) -> Result<(), DomainError>;
    async fn publish_phase_changed(&self, event: &PhaseChangedEvent) -> Result<(), DomainError>;
}
