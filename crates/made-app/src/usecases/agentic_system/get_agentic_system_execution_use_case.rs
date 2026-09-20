//! [`GetAgenticSystemExecutionUseCase`] — what a run intended and
//! what is actually happening.

use std::sync::Arc;

use made_core::entities::{AgenticSystem, AgenticSystemExecution};
use made_core::error::DomainError;
use made_core::ports::{AgenticSystemExecutionStorePort, AgenticSystemPublicationPort};
use made_core::value_objects::AgenticSystemExecutionId;

use super::advance_agentic_system_execution_use_case::Observation;
use super::{AgenticSystemCeremonyView, AgenticSystemExecutionView};

/// Reads a run beside the instances it opened.
///
/// The design and the observation are kept apart in the answer on
/// purpose. A view that merged them would let a ceremony the run
/// skipped read like one the design never had, which is the exact
/// confusion somebody opens this view to resolve.
pub struct GetAgenticSystemExecutionUseCase {
    executions: Arc<dyn AgenticSystemExecutionStorePort>,
    publications: Arc<dyn AgenticSystemPublicationPort>,
    observation: Arc<Observation>,
}

impl std::fmt::Debug for GetAgenticSystemExecutionUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GetAgenticSystemExecutionUseCase")
            .finish()
    }
}

impl GetAgenticSystemExecutionUseCase {
    #[must_use]
    pub const fn new(
        executions: Arc<dyn AgenticSystemExecutionStorePort>,
        publications: Arc<dyn AgenticSystemPublicationPort>,
        observation: Arc<Observation>,
    ) -> Self {
        Self {
            executions,
            publications,
            observation,
        }
    }

    #[tracing::instrument(
        name = "get_agentic_system_execution",
        skip_all,
        fields(execution_id = %id)
    )]
    pub async fn execute(
        &self,
        id: &AgenticSystemExecutionId,
    ) -> Result<AgenticSystemExecutionView, DomainError> {
        let execution = self
            .executions
            .get(id)
            .await?
            .ok_or(DomainError::NotFound {
                what: "agentic_system_execution",
            })?;
        let system = self.sealed_design(&execution).await?;
        let ceremonies = self.ceremonies(&execution).await?;
        Ok(AgenticSystemExecutionView::new(
            execution, system, ceremonies,
        ))
    }

    /// The design as it was sealed, not as it is now.
    ///
    /// Read from the publication store by the run's own pin, so a run
    /// inspected long afterwards is still explained by the document it
    /// actually composed.
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

    async fn ceremonies(
        &self,
        execution: &AgenticSystemExecution,
    ) -> Result<Vec<AgenticSystemCeremonyView>, DomainError> {
        let mut views = Vec::new();
        for (ceremony, link) in execution.ceremonies() {
            let view = AgenticSystemCeremonyView::new(
                ceremony.clone(),
                link.pin().clone(),
                link.status(),
                link.round(),
                link.instance_id().cloned(),
                link.skipped_because().cloned(),
            );
            let observed = match link.instance_id() {
                Some(instance) => self.observation.instance(instance).await?,
                None => None,
            };
            views.push(match observed {
                Some(instance) => {
                    view.observing(instance.lifecycle(), instance.current_state().clone())
                }
                None => view,
            });
        }
        Ok(views)
    }
}
