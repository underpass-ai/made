use async_trait::async_trait;

use crate::entities::CeremonyAgentStatus;
use crate::error::DomainError;
use crate::ports::{CeremonyAgentStatusPage, CeremonyAgentStatusQuery};
use crate::value_objects::AuthorizationEvidence;

#[async_trait]
pub trait CeremonyAgentStatusPort: Send + Sync {
    async fn report(
        &self,
        status: CeremonyAgentStatus,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<CeremonyAgentStatus, DomainError>;
    async fn get(
        &self,
        ceremony_id: &str,
        agent_execution_id: &str,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<CeremonyAgentStatus, DomainError>;
    async fn list(
        &self,
        query: CeremonyAgentStatusQuery,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<CeremonyAgentStatusPage, DomainError>;
}
