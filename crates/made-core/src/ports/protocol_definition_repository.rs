//! [`ProtocolDefinitionRepositoryPort`] — persistence for protocol definitions.

use async_trait::async_trait;

use crate::entities::ProtocolDefinition;
use crate::error::DomainError;
use crate::value_objects::{ProtocolName, ProtocolVersion};

#[async_trait]
pub trait ProtocolDefinitionRepositoryPort: Send + Sync {
    async fn save(&self, definition: &ProtocolDefinition) -> Result<(), DomainError>;

    async fn get(
        &self,
        name: &ProtocolName,
        version: &ProtocolVersion,
    ) -> Result<ProtocolDefinition, DomainError>;

    async fn list(&self) -> Result<Vec<ProtocolDefinition>, DomainError>;
}
