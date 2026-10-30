use made_core::entities::ProtocolAgentStatus;
use made_core::error::DomainError;
use made_core::ports::{ProtocolAgentStatusPage, ProtocolAgentStatusQuery};
use made_core::value_objects::{AuthorizationAction, ProtocolId};

use super::EmbeddedMade;

impl EmbeddedMade {
    pub async fn report_agent_status(
        &self,
        status: ProtocolAgentStatus,
    ) -> Result<ProtocolAgentStatus, DomainError> {
        let protocol_id = status.protocol_id().clone();
        self.require_authorized_protocol_action(
            AuthorizationAction::ReportProtocolAgentStatus,
            &protocol_id,
        )?;
        self.agent_status.report(status).await
    }

    pub async fn get_agent(
        &self,
        protocol_id: &str,
        agent_execution_id: &str,
    ) -> Result<ProtocolAgentStatus, DomainError> {
        let protocol_id = ProtocolId::new(protocol_id.to_owned())?;
        self.require_authorized_protocol_action(
            AuthorizationAction::GetProtocolAgent,
            &protocol_id,
        )?;
        self.agent_status
            .get(protocol_id.as_str(), agent_execution_id)
            .await
    }

    pub async fn list_agents(
        &self,
        query: ProtocolAgentStatusQuery,
    ) -> Result<ProtocolAgentStatusPage, DomainError> {
        let protocol_id = ProtocolId::new(query.protocol_id().to_owned())?;
        self.require_authorized_protocol_action(
            AuthorizationAction::ListProtocolAgents,
            &protocol_id,
        )?;
        self.agent_status.list(query).await
    }
}
