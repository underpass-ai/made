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
use made_core::value_objects::{AgentId, Attributes, OutputContract, OutputContractId, Specialty};

use super::EmbeddedMade;

impl EmbeddedMade {
    pub async fn deliberate(&self, task: Task) -> Result<DeliberateOutput, DomainError> {
        self.councils.deliberate.execute(task).await
    }

    pub async fn stream_deliberation(
        &self,
        task: Task,
        observer: Arc<dyn DeliberationObserverPort>,
    ) -> Result<DeliberateOutput, DomainError> {
        self.councils
            .deliberate
            .execute_with_observer(task, observer)
            .await
    }

    pub async fn get_deliberation_result(
        &self,
        task_id: &made_core::value_objects::TaskId,
    ) -> Result<Deliberation, DomainError> {
        self.councils.get_deliberation.execute(task_id).await
    }

    pub async fn orchestrate(
        &self,
        task: Task,
        execution_options: Attributes,
    ) -> Result<OrchestrateOutput, DomainError> {
        self.councils
            .orchestrate
            .execute(task, execution_options)
            .await
    }

    pub async fn process_trigger_event(
        &self,
        event: &TriggerEvent,
    ) -> Result<AutoDispatchOutcome, DomainError> {
        self.councils.auto_dispatch.dispatch(event).await
    }

    pub async fn run_council_decision(
        &self,
        input: RunCouncilDecisionInput,
    ) -> Result<RunCouncilDecisionOutput, DomainError> {
        self.councils.run_council_decision.execute(input).await
    }

    pub async fn create_council(&self, input: CreateCouncilInput) -> Result<Council, DomainError> {
        self.councils.create_council.execute(input).await
    }

    pub async fn list_councils(&self) -> Result<Vec<Council>, DomainError> {
        self.councils.list_councils.execute().await
    }

    pub async fn delete_council(&self, specialty: &Specialty) -> Result<(), DomainError> {
        self.councils.delete_council.execute(specialty).await
    }

    pub async fn register_agent(
        &self,
        descriptor: AgentDescriptor,
    ) -> Result<AgentId, DomainError> {
        self.councils.register_agent.execute(descriptor).await
    }

    pub async fn unregister_agent(&self, id: &AgentId) -> Result<(), DomainError> {
        self.councils.unregister_agent.execute(id).await
    }

    pub async fn register_contract(&self, contract: OutputContract) -> Result<(), DomainError> {
        self.councils.contracts.register(contract).await
    }

    pub async fn list_contracts(&self) -> Result<Vec<OutputContract>, DomainError> {
        self.councils.contracts.list().await
    }

    pub async fn delete_contract(&self, id: &OutputContractId) -> Result<(), DomainError> {
        self.councils.contracts.delete(id).await
    }
}
