//! [`CeremonyDefinitionSourcePort`] — load ceremony definitions from outside.

use async_trait::async_trait;

use crate::entities::CeremonyDefinition;
use crate::error::DomainError;

#[async_trait]
pub trait CeremonyDefinitionSourcePort: Send + Sync {
    async fn load(&self) -> Result<Vec<CeremonyDefinition>, DomainError>;
}
