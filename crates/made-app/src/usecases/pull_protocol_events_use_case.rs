use std::fmt;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{ProtocolEventCursorPort, ProtocolEventStorePort};
use made_core::value_objects::{GlobalPosition, ProtocolEventPageLimit};

use super::{PullProtocolEventsInput, PullProtocolEventsOutput};

/// Reads a named feed and advances it only when the caller explicitly acknowledges.
pub struct PullProtocolEventsUseCase {
    events: Arc<dyn ProtocolEventStorePort>,
    cursors: Arc<dyn ProtocolEventCursorPort>,
}

impl PullProtocolEventsUseCase {
    #[must_use]
    pub fn new(
        events: Arc<dyn ProtocolEventStorePort>,
        cursors: Arc<dyn ProtocolEventCursorPort>,
    ) -> Self {
        Self { events, cursors }
    }

    pub async fn execute(
        &self,
        input: PullProtocolEventsInput,
    ) -> Result<PullProtocolEventsOutput, DomainError> {
        if let Some(through) = input.acknowledge_through() {
            self.require_existing_position(through).await?;
            self.cursors.acknowledge(input.consumer(), through).await?;
        }
        let acknowledged = self.cursors.position(input.consumer()).await?;
        let from = acknowledged.map_or(GlobalPosition::FIRST, GlobalPosition::next);
        let records = self.events.read_all(from, input.limit()).await?;
        Ok(PullProtocolEventsOutput::new(records, acknowledged))
    }

    async fn require_existing_position(&self, through: GlobalPosition) -> Result<(), DomainError> {
        let one = ProtocolEventPageLimit::new(1)?;
        let exists = self
            .events
            .read_all(through, one)
            .await?
            .first()
            .is_some_and(|record| record.position == through);
        if exists {
            Ok(())
        } else {
            Err(DomainError::NotFound {
                what: "protocol_event_global_position",
            })
        }
    }
}

impl fmt::Debug for PullProtocolEventsUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("PullProtocolEventsUseCase").finish()
    }
}
