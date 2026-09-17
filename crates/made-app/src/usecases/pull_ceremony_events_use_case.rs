use std::fmt;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{CeremonyEventCursorPort, CeremonyEventStorePort};
use made_core::value_objects::{CeremonyEventPageLimit, GlobalPosition};

use super::{PullCeremonyEventsInput, PullCeremonyEventsOutput};

/// Reads a named feed and advances it only when the caller explicitly acknowledges.
pub struct PullCeremonyEventsUseCase {
    events: Arc<dyn CeremonyEventStorePort>,
    cursors: Arc<dyn CeremonyEventCursorPort>,
}

impl PullCeremonyEventsUseCase {
    #[must_use]
    pub fn new(
        events: Arc<dyn CeremonyEventStorePort>,
        cursors: Arc<dyn CeremonyEventCursorPort>,
    ) -> Self {
        Self { events, cursors }
    }

    pub async fn execute(
        &self,
        input: PullCeremonyEventsInput,
    ) -> Result<PullCeremonyEventsOutput, DomainError> {
        if let Some(through) = input.acknowledge_through() {
            self.require_existing_position(through).await?;
            self.cursors.acknowledge(input.consumer(), through).await?;
        }
        let acknowledged = self.cursors.position(input.consumer()).await?;
        let from = acknowledged.map_or(GlobalPosition::FIRST, GlobalPosition::next);
        let records = self.events.read_all(from, input.limit()).await?;
        Ok(PullCeremonyEventsOutput::new(records, acknowledged))
    }

    async fn require_existing_position(&self, through: GlobalPosition) -> Result<(), DomainError> {
        let one = CeremonyEventPageLimit::new(1)?;
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
                what: "ceremony_event_global_position",
            })
        }
    }
}

impl fmt::Debug for PullCeremonyEventsUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("PullCeremonyEventsUseCase").finish()
    }
}
