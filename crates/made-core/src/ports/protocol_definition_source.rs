//! [`ProtocolDefinitionSourcePort`] — load protocol definitions from outside.

use async_trait::async_trait;

use crate::entities::ProtocolDefinition;
use crate::error::DomainError;

#[async_trait]
pub trait ProtocolDefinitionSourcePort: Send + Sync {
    async fn load(&self) -> Result<Vec<ProtocolDefinition>, DomainError>;
}
