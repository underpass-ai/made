use std::sync::Arc;

use made_app::services::AutoDispatchOutcome;
use made_app::usecases::{
    CreateCouncilInput, DeliberateOutput, OrchestrateOutput, RunCouncilDecisionInput,
    RunCouncilDecisionOutput,
};
use made_core::entities::{Council, Deliberation, Task};
use made_core::error::DomainError;
use made_core::events::TriggerEvent;
use made_core::ports::{AgentDescriptor, DeliberationObserverPort};
use made_core::value_objects::{
    AgentId, Attributes, AuthorizationAction, OutputContract, OutputContractId, Specialty,
};

use super::EmbeddedMade;

impl EmbeddedMade {
    pub async fn deliberate(&self, task: Task) -> Result<DeliberateOutput, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::Deliberate)?;
        self.councils.deliberate.execute(task).await
    }

    pub async fn stream_deliberation(
        &self,
        task: Task,
        observer: Arc<dyn DeliberationObserverPort>,
    ) -> Result<DeliberateOutput, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::StreamDeliberation)?;
        self.councils
            .deliberate
            .execute_with_observer(task, observer)
            .await
    }

    pub async fn get_deliberation_result(
        &self,
        task_id: &made_core::value_objects::TaskId,
    ) -> Result<Deliberation, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::GetDeliberationResult)?;
        self.councils.get_deliberation.execute(task_id).await
    }

    pub async fn orchestrate(
        &self,
        task: Task,
        execution_options: Attributes,
    ) -> Result<OrchestrateOutput, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::Orchestrate)?;
        self.councils
            .orchestrate
            .execute(task, execution_options)
            .await
    }

    pub async fn process_trigger_event(
        &self,
        event: &TriggerEvent,
    ) -> Result<AutoDispatchOutcome, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::ProcessTriggerEvent)?;
        self.councils.auto_dispatch.dispatch(event).await
    }

    pub async fn run_council_decision(
        &self,
        input: RunCouncilDecisionInput,
    ) -> Result<RunCouncilDecisionOutput, DomainError> {
        match &input.council_selector {
            made_core::value_objects::CouncilSelector::ById(council_id) => self
                .require_authorized_council_action(
                    AuthorizationAction::RunCouncilDecision,
                    council_id,
                )?,
            made_core::value_objects::CouncilSelector::BySpecialty(_) => {
                self.require_authorized_global_action(AuthorizationAction::RunCouncilDecision)?;
            }
        }
        self.councils.run_council_decision.execute(input).await
    }

    pub async fn create_council(&self, input: CreateCouncilInput) -> Result<Council, DomainError> {
        self.require_authorized_council_action(
            AuthorizationAction::CreateCouncil,
            &input.council_id,
        )?;
        self.councils.create_council.execute(input).await
    }

    pub async fn list_councils(&self) -> Result<Vec<Council>, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::ListCouncils)?;
        self.councils.list_councils.execute().await
    }

    pub async fn delete_council(&self, specialty: &Specialty) -> Result<(), DomainError> {
        self.require_authorized_global_action(AuthorizationAction::DeleteCouncil)?;
        self.councils.delete_council.execute(specialty).await
    }

    pub async fn register_agent(
        &self,
        descriptor: AgentDescriptor,
    ) -> Result<AgentId, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::RegisterAgent)?;
        self.councils.register_agent.execute(descriptor).await
    }

    pub async fn unregister_agent(&self, id: &AgentId) -> Result<(), DomainError> {
        self.require_authorized_global_action(AuthorizationAction::UnregisterAgent)?;
        self.councils.unregister_agent.execute(id).await
    }

    pub async fn register_contract(&self, contract: OutputContract) -> Result<(), DomainError> {
        self.require_authorized_global_action(AuthorizationAction::RegisterContract)?;
        self.councils.contracts.register(contract).await
    }

    pub async fn list_contracts(&self) -> Result<Vec<OutputContract>, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::ListContracts)?;
        self.councils.contracts.list().await
    }

    pub async fn delete_contract(&self, id: &OutputContractId) -> Result<(), DomainError> {
        self.require_authorized_global_action(AuthorizationAction::DeleteContract)?;
        self.councils.contracts.delete(id).await
    }
}
