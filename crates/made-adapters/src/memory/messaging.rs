use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::events::{
    DeliberationCompletedEvent, PhaseChangedEvent, TaskCompletedEvent, TaskDispatchedEvent,
    TaskFailedEvent,
};
use made_core::ports::MessagingPort;
use tokio::sync::RwLock;

use super::InMemoryMessage;

/// In-process event publisher that retains every accepted event.
#[derive(Debug, Default, Clone)]
pub struct InMemoryMessaging {
    events: Arc<RwLock<Vec<InMemoryMessage>>>,
}

impl InMemoryMessaging {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn events(&self) -> Vec<InMemoryMessage> {
        self.events.read().await.clone()
    }

    pub async fn event_kinds(&self) -> Vec<&'static str> {
        self.events
            .read()
            .await
            .iter()
            .map(InMemoryMessage::kind)
            .collect()
    }

    async fn record(&self, event: InMemoryMessage) {
        self.events.write().await.push(event);
    }
}

#[async_trait]
impl MessagingPort for InMemoryMessaging {
    async fn publish_task_dispatched(
        &self,
        event: &TaskDispatchedEvent,
    ) -> Result<(), DomainError> {
        self.record(InMemoryMessage::TaskDispatched(event.clone()))
            .await;
        Ok(())
    }

    async fn publish_task_completed(&self, event: &TaskCompletedEvent) -> Result<(), DomainError> {
        self.record(InMemoryMessage::TaskCompleted(event.clone()))
            .await;
        Ok(())
    }

    async fn publish_task_failed(&self, event: &TaskFailedEvent) -> Result<(), DomainError> {
        self.record(InMemoryMessage::TaskFailed(event.clone()))
            .await;
        Ok(())
    }

    async fn publish_deliberation_completed(
        &self,
        event: &DeliberationCompletedEvent,
    ) -> Result<(), DomainError> {
        self.record(InMemoryMessage::DeliberationCompleted(event.clone()))
            .await;
        Ok(())
    }

    async fn publish_phase_changed(&self, event: &PhaseChangedEvent) -> Result<(), DomainError> {
        self.record(InMemoryMessage::PhaseChanged(event.clone()))
            .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::events::EventEnvelope;
    use made_core::value_objects::{
        AgentId, DurationMs, EventId, ProposalId, Score, Specialty, TaskId,
    };
    use time::macros::datetime;

    #[tokio::test]
    async fn retains_complete_payloads_in_order_across_clones() {
        let messaging = InMemoryMessaging::new();
        let clone = messaging.clone();
        let envelope = EventEnvelope::new(
            EventId::new("event-1").unwrap(),
            datetime!(2026-09-18 12:00:00 UTC),
            "test",
            None,
        )
        .unwrap();
        let dispatched = TaskDispatchedEvent::new(
            envelope.clone(),
            TaskId::new("task-1").unwrap(),
            Specialty::new("review").unwrap(),
            Some(EventId::new("trigger-1").unwrap()),
        );
        let completed = TaskCompletedEvent::new(
            envelope.clone(),
            TaskId::new("task-1").unwrap(),
            Specialty::new("review").unwrap(),
            Some(AgentId::new("agent-1").unwrap()),
            DurationMs::from_millis(10),
        );
        let failed = TaskFailedEvent::new(
            envelope.clone(),
            TaskId::new("task-2").unwrap(),
            Specialty::new("review").unwrap(),
            "provider",
            "unavailable",
        )
        .unwrap();
        let deliberated = DeliberationCompletedEvent::new_with_context(
            envelope.clone(),
            TaskId::new("task-1").unwrap(),
            Specialty::new("review").unwrap(),
            ProposalId::new("proposal-1").unwrap(),
            Score::new(0.75).unwrap(),
            2,
            DurationMs::from_millis(20),
            Some("bundle-1".to_owned()),
        );
        let phase = PhaseChangedEvent::new(
            envelope,
            TaskId::new("task-1").unwrap(),
            "queued",
            "complete",
        )
        .unwrap();

        messaging
            .publish_task_dispatched(&dispatched)
            .await
            .unwrap();
        clone.publish_task_completed(&completed).await.unwrap();
        messaging.publish_task_failed(&failed).await.unwrap();
        clone
            .publish_deliberation_completed(&deliberated)
            .await
            .unwrap();
        messaging.publish_phase_changed(&phase).await.unwrap();

        assert_eq!(
            clone.events().await,
            [
                InMemoryMessage::TaskDispatched(dispatched),
                InMemoryMessage::TaskCompleted(completed),
                InMemoryMessage::TaskFailed(failed),
                InMemoryMessage::DeliberationCompleted(deliberated),
                InMemoryMessage::PhaseChanged(phase),
            ]
        );
        assert_eq!(
            messaging.event_kinds().await,
            [
                "task.dispatched",
                "task.completed",
                "task.failed",
                "deliberation.completed",
                "phase.changed"
            ]
        );
        let InMemoryMessage::DeliberationCompleted(event) = &clone.events().await[3] else {
            panic!("the fourth event is the deliberation completion");
        };
        assert_eq!(event.envelope().event_id().as_str(), "event-1");
        assert_eq!(event.task_id().as_str(), "task-1");
        assert_eq!(event.external_context_bundle_id(), Some("bundle-1"));
    }
}
