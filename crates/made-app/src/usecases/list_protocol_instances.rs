use std::fmt;
use std::sync::Arc;

use made_core::entities::ProtocolInstance;
use made_core::error::DomainError;

use crate::services::SessionStream;

/// Lists every protocol instance the event store holds a stream for,
/// each folded from its stream, in id order.
pub struct ListProtocolInstancesUseCase {
    stream: Arc<SessionStream>,
}

impl fmt::Debug for ListProtocolInstancesUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ListProtocolInstancesUseCase")
            .finish()
    }
}

impl ListProtocolInstancesUseCase {
    #[must_use]
    pub fn new(stream: Arc<SessionStream>) -> Self {
        Self { stream }
    }

    #[tracing::instrument(name = "list_protocol_instances", skip_all)]
    pub async fn execute(&self) -> Result<Vec<ProtocolInstance>, DomainError> {
        let mut instances = Vec::new();
        for id in self.stream.ids().await? {
            instances.push(self.stream.load(&id).await?.instance);
        }
        Ok(instances)
    }
}
