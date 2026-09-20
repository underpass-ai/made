use made_core::entities::CeremonyAgentStatus;
use made_core::error::DomainError;
use made_core::ports::{CeremonyAgentStatusPage, CeremonyAgentStatusQuery};
use made_core::value_objects::{AuthorizationAction, CeremonyId};

use super::EmbeddedMade;

impl EmbeddedMade {
    pub async fn report_agent_status(
        &self,
        status: CeremonyAgentStatus,
    ) -> Result<CeremonyAgentStatus, DomainError> {
        let ceremony_id = status.ceremony_id().clone();
        self.require_authorized_ceremony_action(
            AuthorizationAction::ReportCeremonyAgentStatus,
            &ceremony_id,
        )?;
        self.agent_status.report(status).await
    }

    pub async fn get_agent(
        &self,
        ceremony_id: &str,
        agent_execution_id: &str,
    ) -> Result<CeremonyAgentStatus, DomainError> {
        let ceremony_id = CeremonyId::new(ceremony_id.to_owned())?;
        self.require_authorized_ceremony_action(
            AuthorizationAction::GetCeremonyAgent,
            &ceremony_id,
        )?;
        self.agent_status
            .get(ceremony_id.as_str(), agent_execution_id)
            .await
    }

    pub async fn list_agents(
        &self,
        query: CeremonyAgentStatusQuery,
    ) -> Result<CeremonyAgentStatusPage, DomainError> {
        let ceremony_id = CeremonyId::new(query.ceremony_id().to_owned())?;
        self.require_authorized_ceremony_action(
            AuthorizationAction::ListCeremonyAgents,
            &ceremony_id,
        )?;
        self.agent_status.list(query).await
    }
}
