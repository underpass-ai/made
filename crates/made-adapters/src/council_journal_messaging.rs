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
    immediate_transport: Option<Arc<dyn MessagingPort>>,
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
        Self {
            journal,
            immediate_transport: None,
        }
    }

    /// Optional synchronous embedded-host callback. The journal is committed
    /// first, so a failed or lost callback remains available for replay. This
    /// callback does not advance any consumer cursor; hosts that require
    /// durable delivery should use a leased publisher and deduplicate EventId.
    #[must_use]
    pub fn with_immediate_transport(mut self, transport: Arc<dyn MessagingPort>) -> Self {
        self.immediate_transport = Some(transport);
        self
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
            .map(drop)?;
        match &self.immediate_transport {
            Some(transport) => transport.publish_task_dispatched(event).await,
            None => Ok(()),
        }
    }
    async fn publish_task_completed(&self, event: &TaskCompletedEvent) -> Result<(), DomainError> {
        self.journal
            .publish(CouncilJournalEvent::TaskCompleted(event.clone()))
            .await
            .map(drop)?;
        match &self.immediate_transport {
            Some(transport) => transport.publish_task_completed(event).await,
            None => Ok(()),
        }
    }
    async fn publish_task_failed(&self, event: &TaskFailedEvent) -> Result<(), DomainError> {
        self.journal
            .publish(CouncilJournalEvent::TaskFailed(event.clone()))
            .await
            .map(drop)?;
        match &self.immediate_transport {
            Some(transport) => transport.publish_task_failed(event).await,
            None => Ok(()),
        }
    }
    async fn publish_deliberation_completed(
        &self,
        event: &DeliberationCompletedEvent,
    ) -> Result<(), DomainError> {
        self.journal
            .publish(CouncilJournalEvent::DeliberationCompleted(event.clone()))
            .await
            .map(drop)?;
        match &self.immediate_transport {
            Some(transport) => transport.publish_deliberation_completed(event).await,
            None => Ok(()),
        }
    }
    async fn publish_phase_changed(&self, event: &PhaseChangedEvent) -> Result<(), DomainError> {
        self.journal
            .publish(CouncilJournalEvent::PhaseChanged(event.clone()))
            .await
            .map(drop)?;
        match &self.immediate_transport {
            Some(transport) => transport.publish_phase_changed(event).await,
            None => Ok(()),
        }
    }
}
