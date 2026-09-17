use std::fmt;
use std::sync::Arc;

use made_core::entities::AuditRecord;
use made_core::error::DomainError;
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{CeremonyEventPageLimit, CeremonyId, StreamVersion};

use super::{ReadCeremonyEventsInput, ReadCeremonyEventsUseCase};

/// Reads a whole ceremony stream by following the bounded page use case.
pub struct ReadWholeCeremonyEventsUseCase {
    page: ReadCeremonyEventsUseCase,
}

impl ReadWholeCeremonyEventsUseCase {
    #[must_use]
    pub fn new(events: Arc<dyn CeremonyEventStorePort>) -> Self {
        Self {
            page: ReadCeremonyEventsUseCase::new(events),
        }
    }

    pub async fn execute(&self, ceremony_id: &CeremonyId) -> Result<Vec<AuditRecord>, DomainError> {
        let mut from = StreamVersion::EMPTY;
        let mut records = Vec::new();
        loop {
            let page = self
                .page
                .execute(ReadCeremonyEventsInput::new(
                    ceremony_id.clone(),
                    from,
                    CeremonyEventPageLimit::DEFAULT,
                ))
                .await?;
            let has_more = page.has_more();
            from = page.next_version();
            records.extend(page.into_records());
            if !has_more {
                return Ok(records);
            }
        }
    }
}

impl fmt::Debug for ReadWholeCeremonyEventsUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadWholeCeremonyEventsUseCase")
            .finish()
    }
}
