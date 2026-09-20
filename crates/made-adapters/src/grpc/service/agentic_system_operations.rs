//! The ports an agentic system needs, held once for the service.
//!
//! Ports rather than nine use cases, built per call like the
//! in-process facade does. Nine fields on the service struct would
//! also mean nine builder setters and nine ways for a deployment to
//! be half-wired; one bundle is either configured or honestly absent.

use std::sync::Arc;

use made_app::usecases::agentic_system::{
    AdvanceAgenticSystemExecutionUseCase, AgenticSystemPins, CeremonyLauncher,
    DesignAgenticSystemUseCase, GetAgenticSystemExecutionUseCase, GetAgenticSystemUseCase,
    InstantiateAgenticSystemUseCase, ListAgenticSystemsUseCase, Observation,
    PublishAgenticSystemUseCase, RenderAgenticSystemDiagramUseCase, ValidateAgenticSystemUseCase,
};
use made_core::ports::{
    AgenticSystemDiagramPort, AgenticSystemExecutionStorePort, AgenticSystemPublicationPort,
    AgenticSystemRepositoryPort, CeremonyDefinitionPublicationPort, ClockPort,
    IntegratorBindingPort,
};

/// Everything the nine agentic-system operations are built from.
#[derive(Clone)]
pub struct AgenticSystemOperations {
    repository: Arc<dyn AgenticSystemRepositoryPort>,
    publications: Arc<dyn AgenticSystemPublicationPort>,
    executions: Arc<dyn AgenticSystemExecutionStorePort>,
    diagrams: Arc<dyn AgenticSystemDiagramPort>,
    ceremony_publications: Arc<dyn CeremonyDefinitionPublicationPort>,
    launcher: Arc<CeremonyLauncher>,
    observation: Arc<Observation>,
    bindings: Option<Arc<dyn IntegratorBindingPort>>,
    clock: Arc<dyn ClockPort>,
}

impl AgenticSystemOperations {
    #[allow(clippy::too_many_arguments)] // one bundle, assembled once, at the composition root
    #[must_use]
    pub fn new(
        repository: Arc<dyn AgenticSystemRepositoryPort>,
        publications: Arc<dyn AgenticSystemPublicationPort>,
        executions: Arc<dyn AgenticSystemExecutionStorePort>,
        diagrams: Arc<dyn AgenticSystemDiagramPort>,
        ceremony_publications: Arc<dyn CeremonyDefinitionPublicationPort>,
        launcher: Arc<CeremonyLauncher>,
        observation: Arc<Observation>,
        bindings: Option<Arc<dyn IntegratorBindingPort>>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            repository,
            publications,
            executions,
            diagrams,
            ceremony_publications,
            launcher,
            observation,
            bindings,
            clock,
        }
    }

    pub(crate) fn design(&self) -> DesignAgenticSystemUseCase {
        DesignAgenticSystemUseCase::new(self.repository.clone(), self.pins(), self.clock.clone())
    }

    pub(crate) fn get(&self) -> GetAgenticSystemUseCase {
        GetAgenticSystemUseCase::new(self.repository.clone())
    }

    pub(crate) fn list(&self) -> ListAgenticSystemsUseCase {
        ListAgenticSystemsUseCase::new(self.repository.clone())
    }

    pub(crate) fn validate(&self) -> ValidateAgenticSystemUseCase {
        ValidateAgenticSystemUseCase::new(self.repository.clone(), self.pins())
    }

    pub(crate) fn publish(&self) -> PublishAgenticSystemUseCase {
        PublishAgenticSystemUseCase::new(
            self.repository.clone(),
            self.publications.clone(),
            Arc::new(self.validate()),
            self.clock.clone(),
        )
    }

    pub(crate) fn instantiate(&self) -> InstantiateAgenticSystemUseCase {
        let usecase = InstantiateAgenticSystemUseCase::new(
            self.publications.clone(),
            self.executions.clone(),
            self.launcher.clone(),
            self.clock.clone(),
        );
        match &self.bindings {
            Some(bindings) => usecase.with_integrator_bindings(bindings.clone()),
            None => usecase,
        }
    }

    pub(crate) fn advance(&self) -> AdvanceAgenticSystemExecutionUseCase {
        AdvanceAgenticSystemExecutionUseCase::new(
            self.publications.clone(),
            self.executions.clone(),
            self.launcher.clone(),
            self.observation.clone(),
            self.clock.clone(),
        )
    }

    pub(crate) fn get_execution(&self) -> GetAgenticSystemExecutionUseCase {
        GetAgenticSystemExecutionUseCase::new(
            self.executions.clone(),
            self.publications.clone(),
            self.observation.clone(),
        )
    }

    pub(crate) fn render(&self) -> RenderAgenticSystemDiagramUseCase {
        RenderAgenticSystemDiagramUseCase::new(
            self.repository.clone(),
            self.publications.clone(),
            self.executions.clone(),
            self.diagrams.clone(),
        )
    }

    fn pins(&self) -> Arc<AgenticSystemPins> {
        Arc::new(AgenticSystemPins::new(self.ceremony_publications.clone()))
    }
}

impl std::fmt::Debug for AgenticSystemOperations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgenticSystemOperations")
            .field("activates_integrator", &self.bindings.is_some())
            .finish()
    }
}
