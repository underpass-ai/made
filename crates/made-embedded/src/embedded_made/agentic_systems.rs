//! Designing, sealing and running an agentic system, in process.
//!
//! Every method opens with the same admission check. Authority is
//! global for the whole aggregate in this version (ADR-021): a
//! dedicated scope is a public contract, and one that could not yet be
//! enforced well would be worse than the honest coarse answer.

use std::sync::Arc;

use made_app::usecases::agentic_system::{
    AdvanceAgenticSystemExecutionUseCase, AgenticSystemDesignDocument, AgenticSystemExecutionView,
    AgenticSystemPins, AgenticSystemPublicationView, AgenticSystemValidationView,
    AgenticSystemView, CeremonyLauncher, DesignAgenticSystemUseCase,
    GetAgenticSystemExecutionUseCase, GetAgenticSystemUseCase, InstantiateAgenticSystemInput,
    InstantiateAgenticSystemUseCase, ListAgenticSystemsUseCase, Observation,
    PublishAgenticSystemUseCase, RenderAgenticSystemDiagramUseCase, ValidateAgenticSystemUseCase,
};
use made_app::usecases::{
    BindCeremonyParticipantsUseCase, ResolveCeremonyDefinitionUseCase,
    StartPublishedCeremonyUseCase,
};
use made_core::entities::AgenticSystemExecution;
use made_core::ports::{AgenticSystemDiagram, AgenticSystemPage, AgenticSystemQuery};
use made_core::value_objects::{
    AgenticSystemExecutionId, AgenticSystemId, AgenticSystemRevision, AuditActorId, AuditActorKind,
    AuthorizationAction,
};
use made_core::DomainError;

use super::EmbeddedMade;

impl EmbeddedMade {
    /// Write a system down, or edit the revision you read.
    pub async fn design_agentic_system(
        &self,
        document: AgenticSystemDesignDocument,
    ) -> Result<AgenticSystemView, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::DesignAgenticSystem)?;
        DesignAgenticSystemUseCase::new(
            self.agentic_system.repository().clone(),
            self.agentic_system_pins(),
            self.clock.clone(),
        )
        .execute(document)
        .await
    }

    /// One design, at a named revision or at its head.
    pub async fn get_agentic_system(
        &self,
        id: &AgenticSystemId,
        revision: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystemView, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::GetAgenticSystem)?;
        GetAgenticSystemUseCase::new(self.agentic_system.repository().clone())
            .execute(id, revision)
            .await
    }

    /// One bounded page of the design catalogue.
    pub async fn list_agentic_systems(
        &self,
        query: &AgenticSystemQuery,
    ) -> Result<AgenticSystemPage, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::ListAgenticSystems)?;
        ListAgenticSystemsUseCase::new(self.agentic_system.repository().clone())
            .execute(query)
            .await
    }

    /// Compare a design against the ceremonies it composes.
    pub async fn validate_agentic_system(
        &self,
        id: &AgenticSystemId,
        revision: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystemValidationView, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::ValidateAgenticSystem)?;
        self.validate_use_case().execute(id, revision).await
    }

    /// Seal a revision a run can pin.
    pub async fn publish_agentic_system(
        &self,
        id: &AgenticSystemId,
        revision: AgenticSystemRevision,
    ) -> Result<AgenticSystemPublicationView, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::PublishAgenticSystem)?;
        PublishAgenticSystemUseCase::new(
            self.agentic_system.repository().clone(),
            self.agentic_system.publications().clone(),
            Arc::new(self.validate_use_case()),
            self.clock.clone(),
        )
        .execute(id, revision)
        .await
    }

    /// Open a run of a sealed revision, or hand back the one this
    /// identity already names.
    pub async fn instantiate_agentic_system(
        &self,
        input: InstantiateAgenticSystemInput,
    ) -> Result<AgenticSystemExecution, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::InstantiateAgenticSystem)?;
        let usecase = InstantiateAgenticSystemUseCase::new(
            self.agentic_system.publications().clone(),
            self.agentic_system.executions().clone(),
            Arc::new(self.ceremony_launcher()),
            self.clock.clone(),
        )
        .with_integrator_bindings(self.integrator_bindings().clone());
        Box::pin(usecase.execute(input)).await
    }

    /// Start whatever the run is now ready for.
    pub async fn advance_agentic_system_execution(
        &self,
        execution_id: &AgenticSystemExecutionId,
        actor_id: &AuditActorId,
        actor_kind: AuditActorKind,
    ) -> Result<AgenticSystemExecution, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::AdvanceAgenticSystemExecution)?;
        let usecase = AdvanceAgenticSystemExecutionUseCase::new(
            self.agentic_system.publications().clone(),
            self.agentic_system.executions().clone(),
            Arc::new(self.ceremony_launcher()),
            self.observation(),
            self.clock.clone(),
        );
        Box::pin(usecase.execute(execution_id, actor_id, actor_kind)).await
    }

    /// What a run intended, beside what its instances say.
    pub async fn get_agentic_system_execution(
        &self,
        execution_id: &AgenticSystemExecutionId,
    ) -> Result<AgenticSystemExecutionView, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::GetAgenticSystemExecution)?;
        GetAgenticSystemExecutionUseCase::new(
            self.agentic_system.executions().clone(),
            self.agentic_system.publications().clone(),
            self.observation(),
        )
        .execute(execution_id)
        .await
    }

    /// The topology as Mermaid text and as sentences.
    pub async fn render_agentic_system_diagram(
        &self,
        id: &AgenticSystemId,
        revision: Option<AgenticSystemRevision>,
        execution_id: Option<&AgenticSystemExecutionId>,
    ) -> Result<AgenticSystemDiagram, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::RenderAgenticSystemDiagram)?;
        RenderAgenticSystemDiagramUseCase::new(
            self.agentic_system.repository().clone(),
            self.agentic_system.publications().clone(),
            self.agentic_system.executions().clone(),
            self.agentic_system.diagrams().clone(),
        )
        .execute(id, revision, execution_id)
        .await
    }

    fn agentic_system_pins(&self) -> Arc<AgenticSystemPins> {
        Arc::new(AgenticSystemPins::new(self.publications.clone()))
    }

    fn validate_use_case(&self) -> ValidateAgenticSystemUseCase {
        ValidateAgenticSystemUseCase::new(
            self.agentic_system.repository().clone(),
            self.agentic_system_pins(),
        )
    }

    fn observation(&self) -> Arc<Observation> {
        Arc::new(Observation::new(self.stream.clone()))
    }

    /// Starting a composed ceremony goes through the same use cases a
    /// host would call itself, so a system run and a hand-started
    /// ceremony leave the same records behind.
    fn ceremony_launcher(&self) -> CeremonyLauncher {
        CeremonyLauncher::new(
            Arc::new(StartPublishedCeremonyUseCase::new(
                self.publications.clone(),
                self.stream.clone(),
                self.clock.clone(),
                self.memory_reader.clone(),
            )),
            Arc::new(BindCeremonyParticipantsUseCase::new(
                Arc::new(ResolveCeremonyDefinitionUseCase::new(
                    self.definitions.clone(),
                    self.publications.clone(),
                )),
                self.stream.clone(),
                self.clock.clone(),
            )),
        )
    }
}
