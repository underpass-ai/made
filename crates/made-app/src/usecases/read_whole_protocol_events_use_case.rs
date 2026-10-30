use std::fmt;
use std::sync::Arc;

use made_core::entities::AuditRecord;
use made_core::error::DomainError;
use made_core::ports::ProtocolEventStorePort;
use made_core::value_objects::{ProtocolEventPageLimit, ProtocolId, StreamVersion};

use super::{ReadProtocolEventsInput, ReadProtocolEventsUseCase};

/// Reads the prefix ending at one captured head through the bounded page use case.
pub struct ReadWholeProtocolEventsUseCase {
    events: Arc<dyn ProtocolEventStorePort>,
    page: ReadProtocolEventsUseCase,
}

impl ReadWholeProtocolEventsUseCase {
    #[must_use]
    pub fn new(events: Arc<dyn ProtocolEventStorePort>) -> Self {
        Self {
            events: events.clone(),
            page: ReadProtocolEventsUseCase::new(events),
        }
    }

    pub async fn execute(&self, protocol_id: &ProtocolId) -> Result<Vec<AuditRecord>, DomainError> {
        self.execute_from(protocol_id, StreamVersion::EMPTY).await
    }

    /// Read only the suffix after `from`, up to the head captured by this call.
    pub async fn execute_from(
        &self,
        protocol_id: &ProtocolId,
        mut from: StreamVersion,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        let through = self.events.head(protocol_id).await?;
        if through.is_empty() {
            return Err(DomainError::NotFound {
                what: "protocol_instance",
            });
        }
        let mut records = Vec::new();
        while from < through {
            let remaining = usize::try_from(through.value() - from.value()).unwrap_or(usize::MAX);
            let limit = ProtocolEventPageLimit::new(
                remaining.min(ProtocolEventPageLimit::DEFAULT.value()),
            )?;
            let page = self
                .page
                .execute(ReadProtocolEventsInput::new(
                    protocol_id.clone(),
                    from,
                    limit,
                ))
                .await?;
            if page.records().is_empty() || page.next_version() <= from {
                return Err(DomainError::InvariantViolated {
                    reason: "protocol event page did not advance to the captured head",
                });
            }
            from = page.next_version();
            records.extend(page.into_records());
        }
        Ok(records)
    }
}

impl fmt::Debug for ReadWholeProtocolEventsUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadWholeProtocolEventsUseCase")
            .finish()
    }
}
