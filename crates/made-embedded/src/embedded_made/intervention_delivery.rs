//! Putting an intervention in front of a live agent, and reading back
//! whether it got there.
//!
//! Four capabilities in their own file rather than beside the other
//! participation verbs, because these four answer a different question.
//! The verbs next door are about what a seat says; these are about
//! whether what was said reached anybody, and the evidence for that
//! comes out of the delivery ledger rather than out of anyone's claim.

use made_app::usecases::{
    AcknowledgeProtocolAgentInterventionInput, AcknowledgeProtocolAgentInterventionUseCase,
    GetProtocolInterventionInput, GetProtocolInterventionUseCase, ListProtocolInterventionsInput,
    ListProtocolInterventionsUseCase, ProtocolInterventionPage, ProtocolInterventionView,
    PullProtocolAgentInterventionsInput, PullProtocolAgentInterventionsUseCase,
    PulledProtocolInterventions,
};
use made_core::entities::ProtocolInstance;
use made_core::value_objects::AuthorizationAction;
use made_core::DomainError;

use super::EmbeddedMade;

impl EmbeddedMade {
    /// Hand a working agent the questions it has been asked.
    ///
    /// Under a lease, which is an offer and not a receipt: what comes
    /// back is what the agent may now answer, and the engine still does
    /// not know it read any of it.
    pub async fn pull_agent_interventions(
        &self,
        input: PullProtocolAgentInterventionsInput,
    ) -> Result<PulledProtocolInterventions, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::PullProtocolAgentInterventions,
            input.instance_id(),
        )?;
        PullProtocolAgentInterventionsUseCase::new(
            self.stream.clone(),
            self.agent_status.clone(),
            self.host_delivery_ledger().clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    /// Seal what an agent says it saw of an offer it was handed.
    pub async fn acknowledge_agent_intervention(
        &self,
        input: AcknowledgeProtocolAgentInterventionInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::AcknowledgeProtocolAgentIntervention,
            input.instance_id(),
        )?;
        AcknowledgeProtocolAgentInterventionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.host_delivery_ledger().clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    /// One intervention, with every route it took and where it stands.
    pub async fn get_intervention(
        &self,
        input: GetProtocolInterventionInput,
    ) -> Result<ProtocolInterventionView, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::GetProtocolIntervention,
            input.instance_id(),
        )?;
        GetProtocolInterventionUseCase::new(
            self.stream.clone(),
            self.host_delivery_ledger().clone(),
        )
        .execute(input)
        .await
    }

    /// What a protocol has been asked, and what is still owed.
    pub async fn list_interventions(
        &self,
        input: ListProtocolInterventionsInput,
    ) -> Result<ProtocolInterventionPage, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::ListProtocolInterventions,
            input.instance_id(),
        )?;
        ListProtocolInterventionsUseCase::new(
            self.stream.clone(),
            self.host_delivery_ledger().clone(),
        )
        .execute(input)
        .await
    }
}
