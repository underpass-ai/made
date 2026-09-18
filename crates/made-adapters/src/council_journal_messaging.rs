use async_trait::async_trait;
use made_core::entities::CouncilJournalEvent;
use made_core::error::DomainError;
use made_core::events::{
    DeliberationCompletedEvent, PhaseChangedEvent, TaskCompletedEvent, TaskDispatchedEvent,
    TaskFailedEvent,
};
use made_core::ports::{CouncilJournalPort, MessagingPort};
use std::sync::Arc;

/// Council messaging writes to its durable journal before any external delivery.
/// A separate leased consumer may forward these immutable facts to a broker.
pub struct CouncilJournalMessaging {
    journal: Arc<dyn CouncilJournalPort>,
}
impl std::fmt::Debug for CouncilJournalMessaging {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CouncilJournalMessaging")
            .finish_non_exhaustive()
    }
}
impl CouncilJournalMessaging {
    #[must_use]
    pub fn new(journal: Arc<dyn CouncilJournalPort>) -> Self {
        Self { journal }
    }
}
#[async_trait]
impl MessagingPort for CouncilJournalMessaging {
    async fn publish_task_dispatched(
        &self,
        event: &TaskDispatchedEvent,
    ) -> Result<(), DomainError> {
        self.journal
            .publish(CouncilJournalEvent::TaskDispatched(event.clone()))
            .await
            .map(drop)
    }
    async fn publish_task_completed(&self, event: &TaskCompletedEvent) -> Result<(), DomainError> {
        self.journal
            .publish(CouncilJournalEvent::TaskCompleted(event.clone()))
            .await
            .map(drop)
    }
    async fn publish_task_failed(&self, event: &TaskFailedEvent) -> Result<(), DomainError> {
        self.journal
            .publish(CouncilJournalEvent::TaskFailed(event.clone()))
            .await
            .map(drop)
    }
    async fn publish_deliberation_completed(
        &self,
        event: &DeliberationCompletedEvent,
    ) -> Result<(), DomainError> {
        self.journal
            .publish(CouncilJournalEvent::DeliberationCompleted(event.clone()))
            .await
            .map(drop)
    }
    async fn publish_phase_changed(&self, event: &PhaseChangedEvent) -> Result<(), DomainError> {
        self.journal
            .publish(CouncilJournalEvent::PhaseChanged(event.clone()))
            .await
            .map(drop)
    }
}
