//! [`RenderAgenticSystemDiagramUseCase`] — the topology, drawn.

use std::sync::Arc;

use made_core::entities::{AgenticSystem, AgenticSystemExecution};
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemDiagram, AgenticSystemDiagramPort, AgenticSystemExecutionStorePort,
    AgenticSystemPublicationPort, AgenticSystemRepositoryPort,
};
use made_core::value_objects::{AgenticSystemExecutionId, AgenticSystemId, AgenticSystemRevision};

/// Draws a design, or a run of one.
///
/// Rendering goes through a port because a diagram is a wire shape:
/// which arrow style means which kind of collaboration is a domain
/// fact, and what that looks like is not.
pub struct RenderAgenticSystemDiagramUseCase {
    repository: Arc<dyn AgenticSystemRepositoryPort>,
    publications: Arc<dyn AgenticSystemPublicationPort>,
    executions: Arc<dyn AgenticSystemExecutionStorePort>,
    diagrams: Arc<dyn AgenticSystemDiagramPort>,
}

impl std::fmt::Debug for RenderAgenticSystemDiagramUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RenderAgenticSystemDiagramUseCase")
            .finish()
    }
}

impl RenderAgenticSystemDiagramUseCase {
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

    /// One design, optionally as one run of it is going.
    ///
    /// With a run, the design comes from what that run pinned rather
    /// than from the head: drawing today's design with yesterday's
    /// progress on it would be a picture of something that never
    /// existed.
    #[tracing::instrument(name = "render_agentic_system_diagram", skip_all)]
    pub async fn execute(
        &self,
        id: &AgenticSystemId,
        revision: Option<AgenticSystemRevision>,
        execution_id: Option<&AgenticSystemExecutionId>,
    ) -> Result<AgenticSystemDiagram, DomainError> {
        let execution = match execution_id {
            Some(execution_id) => Some(self.run(execution_id).await?),
            None => None,
        };
        let system = match &execution {
            Some(execution) => self.sealed_design(execution).await?,
            None => self.design(id, revision).await?,
        };
        self.diagrams.render(&system, execution.as_ref())
    }

    async fn design(
        &self,
        id: &AgenticSystemId,
        revision: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystem, DomainError> {
        self.repository
            .get(id, revision)
            .await?
            .ok_or(DomainError::NotFound {
                what: "agentic_system",
            })
    }

    async fn run(
        &self,
        execution_id: &AgenticSystemExecutionId,
    ) -> Result<AgenticSystemExecution, DomainError> {
        self.executions
            .get(execution_id)
            .await?
            .ok_or(DomainError::NotFound {
                what: "agentic_system_execution",
            })
    }

    async fn sealed_design(
        &self,
        execution: &AgenticSystemExecution,
    ) -> Result<AgenticSystem, DomainError> {
        let published = self
            .publications
            .published(execution.system().id(), execution.system().revision())
            .await?
            .ok_or(DomainError::NotFound {
                what: "published_agentic_system",
            })?;
        Ok(published.into_system())
    }
}
