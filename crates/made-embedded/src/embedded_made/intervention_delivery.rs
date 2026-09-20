//! Putting an intervention in front of a live agent, and reading back
//! whether it got there.
//!
//! Four capabilities in their own file rather than beside the other
//! participation verbs, because these four answer a different question.
//! The verbs next door are about what a seat says; these are about
//! whether what was said reached anybody, and the evidence for that
//! comes out of the delivery ledger rather than out of anyone's claim.

use made_app::usecases::{
    AcknowledgeCeremonyAgentInterventionInput, AcknowledgeCeremonyAgentInterventionUseCase,
    CeremonyInterventionPage, CeremonyInterventionView, GetCeremonyInterventionInput,
    GetCeremonyInterventionUseCase, ListCeremonyInterventionsInput,
    ListCeremonyInterventionsUseCase, PullCeremonyAgentInterventionsInput,
    PullCeremonyAgentInterventionsUseCase, PulledCeremonyInterventions,
};
use made_core::entities::CeremonyInstance;
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
        input: PullCeremonyAgentInterventionsInput,
    ) -> Result<PulledCeremonyInterventions, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::PullCeremonyAgentInterventions,
            input.instance_id(),
        )?;
        PullCeremonyAgentInterventionsUseCase::new(
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
        input: AcknowledgeCeremonyAgentInterventionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::AcknowledgeCeremonyAgentIntervention,
            input.instance_id(),
        )?;
        AcknowledgeCeremonyAgentInterventionUseCase::new(
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
        input: GetCeremonyInterventionInput,
    ) -> Result<CeremonyInterventionView, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::GetCeremonyIntervention,
            input.instance_id(),
        )?;
        GetCeremonyInterventionUseCase::new(self.stream.clone(), self.host_delivery_ledger().clone())
            .execute(input)
            .await
    }

    /// What a ceremony has been asked, and what is still owed.
    pub async fn list_interventions(
        &self,
        input: ListCeremonyInterventionsInput,
    ) -> Result<CeremonyInterventionPage, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::ListCeremonyInterventions,
            input.instance_id(),
        )?;
        ListCeremonyInterventionsUseCase::new(
            self.stream.clone(),
            self.host_delivery_ledger().clone(),
        )
        .execute(input)
        .await
    }
}
