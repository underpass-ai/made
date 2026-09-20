//! [`AdvanceAgenticSystemExecutionUseCase`] — move a run forward.

use std::collections::BTreeSet;
use std::sync::Arc;

use made_core::entities::{AgenticSystem, AgenticSystemExecution, PublishedAgenticSystem};
use made_core::error::DomainError;
use made_core::ports::{AgenticSystemExecutionStorePort, AgenticSystemPublicationPort, ClockPort};
use made_core::value_objects::{
    AgenticSystemExecutionId, AuditActorId, AuditActorKind, CeremonyExecutionLink, SystemCeremonyId,
};

use super::{materialization, CeremonyLauncher};

mod observation;
mod rounds;

pub use observation::Observation;

/// Starts whatever the run is now ready for, and sends bounded loops
/// round again.
///
/// Idempotent by construction: what can start is derived from what the
/// instances say rather than from a cursor, and each instance has a
/// deterministic identity, so advancing twice with nothing having
/// changed starts nothing twice.
pub struct AdvanceAgenticSystemExecutionUseCase {
    publications: Arc<dyn AgenticSystemPublicationPort>,
    executions: Arc<dyn AgenticSystemExecutionStorePort>,
    launcher: Arc<CeremonyLauncher>,
    observation: Arc<Observation>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for AdvanceAgenticSystemExecutionUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdvanceAgenticSystemExecutionUseCase")
            .finish()
    }
}

impl AdvanceAgenticSystemExecutionUseCase {
    #[must_use]
    pub const fn new(
        publications: Arc<dyn AgenticSystemPublicationPort>,
        executions: Arc<dyn AgenticSystemExecutionStorePort>,
        launcher: Arc<CeremonyLauncher>,
        observation: Arc<Observation>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            publications,
            executions,
            launcher,
            observation,
            clock,
        }
    }

    #[tracing::instrument(
        name = "advance_agentic_system_execution",
        skip_all,
        fields(execution_id = %execution_id)
    )]
    pub async fn execute(
        &self,
        execution_id: &AgenticSystemExecutionId,
        actor_id: &AuditActorId,
        actor_kind: AuditActorKind,
    ) -> Result<AgenticSystemExecution, DomainError> {
        let before = self
            .executions
            .get(execution_id)
            .await?
            .ok_or(DomainError::NotFound {
                what: "agentic_system_execution",
            })?;
        let published = self.published(&before).await?;
        let system = published.system();

        // What the instances say comes first: a run decides what to
        // start from observed completions, never from what it hoped
        // had happened by now.
        let mut execution = self.observation.settle(before, system).await?;
        execution = self
            .start_ready(execution, system, actor_id, actor_kind)
            .await?;
        execution = rounds::reopen(&execution, system, self.clock.now())?;
        execution = self
            .start_ready(execution, system, actor_id, actor_kind)
            .await?;
        self.store(execution).await
    }

    async fn published(
        &self,
        execution: &AgenticSystemExecution,
    ) -> Result<PublishedAgenticSystem, DomainError> {
        self.publications
            .published(execution.system().id(), execution.system().revision())
            .await?
            .ok_or(DomainError::NotFound {
                what: "published_agentic_system",
            })
    }

    async fn start_ready(
        &self,
        mut execution: AgenticSystemExecution,
        system: &AgenticSystem,
        actor_id: &AuditActorId,
        actor_kind: AuditActorKind,
    ) -> Result<AgenticSystemExecution, DomainError> {
        let blocking = system.blocking_dependencies();
        let ready: Vec<SystemCeremonyId> = execution
            .ready(&blocking)
            .into_iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        for ceremony in ready {
            let link = self
                .begin(&execution, system, &ceremony, actor_id, actor_kind)
                .await?;
            execution = execution.with_link(&ceremony, link, self.clock.now())?;
        }
        Ok(execution)
    }

    /// Start one composition, or record why it was skipped.
    ///
    /// A ceremony whose participants the host never supplied is
    /// skipped with the reason, in every round. Nothing is stood in
    /// for, at instantiation or later.
    async fn begin(
        &self,
        execution: &AgenticSystemExecution,
        system: &AgenticSystem,
        ceremony: &SystemCeremonyId,
        actor_id: &AuditActorId,
        actor_kind: AuditActorKind,
    ) -> Result<CeremonyExecutionLink, DomainError> {
        let link = execution.link(ceremony).ok_or(DomainError::NotFound {
            what: "agentic_system_execution.ceremony",
        })?;
        let materialized = execution.participants();
        if let Some(reason) = materialization::blocking(system, materialized, ceremony)? {
            return Ok(link.skipped(reason));
        }
        let composition = &system.ceremonies()[ceremony];
        let round = link.round().next();
        let context = self
            .observation
            .projected_inputs(execution, composition)
            .await?;
        let instance = self
            .launcher
            .launch(
                execution.id(),
                composition,
                round,
                context,
                materialized,
                actor_id,
                actor_kind,
            )
            .await?;
        Ok(link.started(instance, round))
    }

    async fn store(
        &self,
        execution: AgenticSystemExecution,
    ) -> Result<AgenticSystemExecution, DomainError> {
        let stored = self
            .executions
            .get(execution.id())
            .await?
            .ok_or(DomainError::NotFound {
                what: "agentic_system_execution",
            })?;
        Ok(self
            .executions
            .update(execution, stored.updated_at())
            .await?
            .into_execution())
    }
}
