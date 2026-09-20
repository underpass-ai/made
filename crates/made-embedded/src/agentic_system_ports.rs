use std::fmt;
use std::sync::Arc;

use made_core::ports::{
    AgenticSystemDiagramPort, AgenticSystemExecutionStorePort, AgenticSystemPublicationPort,
    AgenticSystemRepositoryPort,
};

/// The four ports an agentic system travels through, composed as one.
///
/// One field rather than four because they are never chosen
/// separately: a durable revision log beside an in-memory publication
/// store would restart into a host that had forgotten which revisions
/// its runs are pinned to.
#[derive(Clone)]
pub struct AgenticSystemPorts {
    repository: Arc<dyn AgenticSystemRepositoryPort>,
    publications: Arc<dyn AgenticSystemPublicationPort>,
    executions: Arc<dyn AgenticSystemExecutionStorePort>,
    diagrams: Arc<dyn AgenticSystemDiagramPort>,
}

impl AgenticSystemPorts {
    #[must_use]
    pub const fn new(
        repository: Arc<dyn AgenticSystemRepositoryPort>,
        publications: Arc<dyn AgenticSystemPublicationPort>,
        executions: Arc<dyn AgenticSystemExecutionStorePort>,
        diagrams: Arc<dyn AgenticSystemDiagramPort>,
    ) -> Self {
        Self {
            repository,
            publications,
            executions,
            diagrams,
        }
    }

    /// Where designs and their revisions live.
    #[must_use]
    pub fn repository(&self) -> &Arc<dyn AgenticSystemRepositoryPort> {
        &self.repository
    }

    /// Where sealed revisions live.
    #[must_use]
    pub fn publications(&self) -> &Arc<dyn AgenticSystemPublicationPort> {
        &self.publications
    }

    /// Where runs live.
    #[must_use]
    pub fn executions(&self) -> &Arc<dyn AgenticSystemExecutionStorePort> {
        &self.executions
    }

    /// What draws the topology.
    #[must_use]
    pub fn diagrams(&self) -> &Arc<dyn AgenticSystemDiagramPort> {
        &self.diagrams
    }
}

impl fmt::Debug for AgenticSystemPorts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("AgenticSystemPorts").finish()
    }
}
