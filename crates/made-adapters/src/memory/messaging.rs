use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::events::{
    DeliberationCompletedEvent, PhaseChangedEvent, TaskCompletedEvent, TaskDispatchedEvent,
    TaskFailedEvent,
};
use made_core::ports::MessagingPort;
use tokio::sync::RwLock;

/// In-process event publisher that retains every accepted event.
#[derive(Debug, Default, Clone)]
pub struct InMemoryMessaging {
    events: Arc<RwLock<Vec<String>>>,
}

impl InMemoryMessaging {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn event_kinds(&self) -> Vec<String> {
        self.events.read().await.clone()
    }

    async fn record(&self, kind: &str) {
        self.events.write().await.push(kind.to_owned());
    }
}

#[async_trait]
impl MessagingPort for InMemoryMessaging {
    async fn publish_task_dispatched(
        &self,
        _event: &TaskDispatchedEvent,
    ) -> Result<(), DomainError> {
        self.record("task.dispatched").await;
        Ok(())
    }

    async fn publish_task_completed(&self, _event: &TaskCompletedEvent) -> Result<(), DomainError> {
        self.record("task.completed").await;
        Ok(())
    }

    async fn publish_task_failed(&self, _event: &TaskFailedEvent) -> Result<(), DomainError> {
        self.record("task.failed").await;
        Ok(())
    }

    async fn publish_deliberation_completed(
        &self,
        _event: &DeliberationCompletedEvent,
    ) -> Result<(), DomainError> {
        self.record("deliberation.completed").await;
        Ok(())
    }

    async fn publish_phase_changed(&self, _event: &PhaseChangedEvent) -> Result<(), DomainError> {
        self.record("phase.changed").await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::events::EventEnvelope;
    use made_core::value_objects::{EventId, Specialty, TaskId};
    use time::macros::datetime;

    #[tokio::test]
    async fn records_published_events_in_order() {
        let messaging = InMemoryMessaging::new();
        let envelope = EventEnvelope::new(
            EventId::new("event-1").unwrap(),
            datetime!(2026-09-18 12:00:00 UTC),
            "test",
            None,
        )
        .unwrap();
        messaging
            .publish_task_dispatched(&TaskDispatchedEvent::new(
                envelope,
                TaskId::new("task-1").unwrap(),
                Specialty::new("review").unwrap(),
                None,
            ))
            .await
            .unwrap();
        assert_eq!(messaging.event_kinds().await, ["task.dispatched"]);
    }
}
