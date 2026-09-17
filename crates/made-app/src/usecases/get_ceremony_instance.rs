use std::fmt;
use std::sync::Arc;

use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::value_objects::{CeremonyId, TraceId};

use crate::services::SessionStream;
use crate::usecases::CeremonyInstanceRead;

/// Retrieves a ceremony instance as the fold of its stream.
pub struct GetCeremonyInstanceUseCase {
    stream: Arc<SessionStream>,
}

impl fmt::Debug for GetCeremonyInstanceUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GetCeremonyInstanceUseCase")
            .finish()
    }
}

impl GetCeremonyInstanceUseCase {
    #[must_use]
    pub fn new(stream: Arc<SessionStream>) -> Self {
        Self { stream }
    }

    #[tracing::instrument(name = "get_ceremony_instance", skip_all, fields(ceremony_id = %id))]
    pub async fn execute(&self, id: &CeremonyId) -> Result<CeremonyInstance, DomainError> {
        Ok(self.stream.load(id).await?.instance)
    }

    /// Retrieve the fold and the identity fields of the exact head it came from.
    pub async fn execute_with_ids(
        &self,
        id: &CeremonyId,
    ) -> Result<CeremonyInstanceRead, DomainError> {
        let records = self.stream.records(id).await?;
        let head = records.last();
        let trace_id = head
            .and_then(|record| record.trace_id())
            .map(TraceId::new)
            .transpose()?;
        let read = CeremonyInstanceRead::new(
            SessionStream::fold_records(&records)?.instance,
            trace_id,
            head.and_then(|record| record.correlation_id()).cloned(),
            head.and_then(|record| record.causation_id()).cloned(),
        );
        Ok(read)
    }
}
