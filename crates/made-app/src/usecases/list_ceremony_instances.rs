use std::fmt;
use std::sync::Arc;

use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;

use crate::services::SessionStream;

/// Lists every ceremony instance the event store holds a stream for,
/// each folded from its stream, in id order.
pub struct ListCeremonyInstancesUseCase {
    stream: Arc<SessionStream>,
}

impl fmt::Debug for ListCeremonyInstancesUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ListCeremonyInstancesUseCase")
            .finish()
    }
}

impl ListCeremonyInstancesUseCase {
    #[must_use]
    pub fn new(stream: Arc<SessionStream>) -> Self {
        Self { stream }
    }

    #[tracing::instrument(name = "list_ceremony_instances", skip_all)]
    pub async fn execute(&self) -> Result<Vec<CeremonyInstance>, DomainError> {
        let mut instances = Vec::new();
        for id in self.stream.ids().await? {
            instances.push(self.stream.load(&id).await?.instance);
        }
        Ok(instances)
    }
}
