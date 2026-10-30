use async_trait::async_trait;

use crate::entities::ProtocolAgentStatus;
use crate::error::DomainError;
use crate::ports::{
    ProtocolAgentActivityPage, ProtocolAgentActivityQuery, ProtocolAgentActivitySubscriptionPort,
    ProtocolAgentStatusPage, ProtocolAgentStatusQuery,
};
use crate::value_objects::AuthorizationEvidence;

#[async_trait]
pub trait ProtocolAgentStatusPort: Send + Sync {
    fn subscribe_activity(&self) -> Box<dyn ProtocolAgentActivitySubscriptionPort>;
    async fn report(
        &self,
        status: ProtocolAgentStatus,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ProtocolAgentStatus, DomainError>;
    async fn get(
        &self,
        protocol_id: &str,
        agent_execution_id: &str,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ProtocolAgentStatus, DomainError>;
    async fn list(
        &self,
        query: ProtocolAgentStatusQuery,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ProtocolAgentStatusPage, DomainError>;
    async fn read_activity(
        &self,
        query: ProtocolAgentActivityQuery,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ProtocolAgentActivityPage, DomainError>;
}
