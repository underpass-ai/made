use std::fmt;
use std::sync::Arc;

use made_adapters::memory::{
    InMemoryAgenticSystemExecutions, InMemoryAgenticSystemPublications,
    InMemoryAgenticSystemRepository,
};
use made_adapters::mermaid::AgenticSystemMermaidDiagram;
use made_core::ports::{
    AgenticSystemDiagramPort, AgenticSystemExecutionStorePort, AgenticSystemPublicationPort,
    AgenticSystemRepositoryPort,
};

use crate::agentic_system_ports::AgenticSystemPorts;
use crate::EmbeddedMadeBuilder;

/// The agentic-system adapters a host has chosen, before defaults
/// fill the rest.
#[derive(Default)]
pub(crate) struct AgenticSystemWiring {
    repository: Option<Arc<dyn AgenticSystemRepositoryPort>>,
    publications: Option<Arc<dyn AgenticSystemPublicationPort>>,
    executions: Option<Arc<dyn AgenticSystemExecutionStorePort>>,
    diagrams: Option<Arc<dyn AgenticSystemDiagramPort>>,
}

impl AgenticSystemWiring {
    /// Fill what the host did not choose.
    ///
    /// In memory: a process that forgets its designs on restart is a
    /// normal embedded host, and saying so beats pretending a draft
    /// somebody spent an afternoon on is safe where it is not.
    pub(crate) fn resolve(self) -> AgenticSystemPorts {
        AgenticSystemPorts::new(
            self.repository
                .unwrap_or_else(|| Arc::new(InMemoryAgenticSystemRepository::new())),
            self.publications
                .unwrap_or_else(|| Arc::new(InMemoryAgenticSystemPublications::new())),
            self.executions
                .unwrap_or_else(|| Arc::new(InMemoryAgenticSystemExecutions::new())),
            self.diagrams
                .unwrap_or_else(|| Arc::new(AgenticSystemMermaidDiagram::new())),
        )
    }
}

impl EmbeddedMadeBuilder {
    /// Where designs, their sealed revisions and their runs are held.
    ///
    /// Taken together rather than one at a time: the three are only
    /// meaningful as a set, and a host that made them durable one call
    /// at a time could leave two of them agreeing about a design the
    /// third had never heard of.
    #[must_use]
    pub fn with_agentic_system_stores(
        mut self,
        repository: Arc<dyn AgenticSystemRepositoryPort>,
        publications: Arc<dyn AgenticSystemPublicationPort>,
        executions: Arc<dyn AgenticSystemExecutionStorePort>,
    ) -> Self {
        self.agentic_system.repository = Some(repository);
        self.agentic_system.publications = Some(publications);
        self.agentic_system.executions = Some(executions);
        self
    }

    /// What draws a system's topology.
    #[must_use]
    pub fn with_agentic_system_diagrams(
        mut self,
        adapter: Arc<dyn AgenticSystemDiagramPort>,
    ) -> Self {
        self.agentic_system.diagrams = Some(adapter);
        self
    }
}

impl fmt::Debug for AgenticSystemWiring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgenticSystemWiring")
            .field("has_repository", &self.repository.is_some())
            .field("has_publications", &self.publications.is_some())
            .field("has_executions", &self.executions.is_some())
            .field("has_diagrams", &self.diagrams.is_some())
            .finish()
    }
}
