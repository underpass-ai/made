use std::fmt;
use std::sync::Arc;

use made_core::entities::ProtocolInstance;
use made_core::error::DomainError;
use made_core::value_objects::{ProtocolId, TraceId};

use crate::services::SessionStream;
use crate::usecases::ProtocolInstanceRead;

/// Retrieves a protocol instance as the fold of its stream.
pub struct GetProtocolInstanceUseCase {
    stream: Arc<SessionStream>,
}

impl fmt::Debug for GetProtocolInstanceUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GetProtocolInstanceUseCase")
            .finish()
    }
}

impl GetProtocolInstanceUseCase {
    #[must_use]
    pub fn new(stream: Arc<SessionStream>) -> Self {
        Self { stream }
    }

    #[tracing::instrument(name = "get_protocol_instance", skip_all, fields(protocol_id = %id))]
    pub async fn execute(&self, id: &ProtocolId) -> Result<ProtocolInstance, DomainError> {
        Ok(self.stream.load(id).await?.instance)
    }

    /// Retrieve the fold and the identity fields of the exact head it came from.
    pub async fn execute_with_ids(
        &self,
        id: &ProtocolId,
    ) -> Result<ProtocolInstanceRead, DomainError> {
        let records = self.stream.records(id).await?;
        let head = records.last();
        let trace_id = head
            .and_then(|record| record.trace_id())
            .map(TraceId::new)
            .transpose()?;
        let read = ProtocolInstanceRead::new(
            SessionStream::fold_records(&records)?.instance,
            trace_id,
            head.and_then(|record| record.correlation_id()).cloned(),
            head.and_then(|record| record.causation_id()).cloned(),
        );
        Ok(read)
    }
}
