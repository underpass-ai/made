use std::fmt;
use std::sync::Arc;

use made_core::entities::AuditRecord;
use made_core::error::DomainError;
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{CeremonyEventPageLimit, CeremonyId, StreamVersion};

use super::{ReadCeremonyEventsInput, ReadCeremonyEventsUseCase};

/// Reads the prefix ending at one captured head through the bounded page use case.
pub struct ReadWholeCeremonyEventsUseCase {
    events: Arc<dyn CeremonyEventStorePort>,
    page: ReadCeremonyEventsUseCase,
}

impl ReadWholeCeremonyEventsUseCase {
    #[must_use]
    pub fn new(events: Arc<dyn CeremonyEventStorePort>) -> Self {
        Self {
            events: events.clone(),
            page: ReadCeremonyEventsUseCase::new(events),
        }
    }

    pub async fn execute(&self, ceremony_id: &CeremonyId) -> Result<Vec<AuditRecord>, DomainError> {
        self.execute_from(ceremony_id, StreamVersion::EMPTY).await
    }

    /// Read only the suffix after `from`, up to the head captured by this call.
    pub async fn execute_from(
        &self,
        ceremony_id: &CeremonyId,
        mut from: StreamVersion,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        let through = self.events.head(ceremony_id).await?;
        if through.is_empty() {
            return Err(DomainError::NotFound {
                what: "ceremony_instance",
            });
        }
        let mut records = Vec::new();
        while from < through {
            let remaining = usize::try_from(through.value() - from.value()).unwrap_or(usize::MAX);
            let limit = CeremonyEventPageLimit::new(
                remaining.min(CeremonyEventPageLimit::DEFAULT.value()),
            )?;
            let page = self
                .page
                .execute(ReadCeremonyEventsInput::new(
                    ceremony_id.clone(),
                    from,
                    limit,
                ))
                .await?;
            if page.records().is_empty() || page.next_version() <= from {
                return Err(DomainError::InvariantViolated {
                    reason: "ceremony event page did not advance to the captured head",
                });
            }
            from = page.next_version();
            records.extend(page.into_records());
        }
        Ok(records)
    }
}

impl fmt::Debug for ReadWholeCeremonyEventsUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadWholeCeremonyEventsUseCase")
            .finish()
    }
}
