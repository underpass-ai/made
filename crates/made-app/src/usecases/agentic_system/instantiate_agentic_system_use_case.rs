//! [`InstantiateAgenticSystemUseCase`] — run a sealed design.

use std::collections::BTreeMap;
use std::sync::Arc;

use made_core::entities::{AgenticSystem, AgenticSystemExecution, PublishedAgenticSystem};
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemExecutionCreation, AgenticSystemExecutionStorePort, AgenticSystemPublicationPort,
    BindReplacement, ClockPort, IntegratorBindingPort,
};
use made_core::value_objects::{
    CeremonyExecutionLink, HostAgentIncarnation, IntegratorBinding, IntegratorBindingId,
    IntegratorScope, LoopRound, ParticipantMaterialization, RoleId, SystemCeremonyId,
};

use super::{materialization, CeremonyLauncher, InstantiateAgenticSystemInput};

/// Opens a run of one published revision and starts what can start.
///
/// Idempotent by the caller's execution identity: asking twice hands
/// back the run that exists, after checking it is running the same
/// design. A host that did not hear the first answer must be able to
/// ask again without starting the work twice.
pub struct InstantiateAgenticSystemUseCase {
    publications: Arc<dyn AgenticSystemPublicationPort>,
    executions: Arc<dyn AgenticSystemExecutionStorePort>,
    launcher: Arc<CeremonyLauncher>,
    bindings: Option<Arc<dyn IntegratorBindingPort>>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for InstantiateAgenticSystemUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstantiateAgenticSystemUseCase")
            .finish()
    }
}

impl InstantiateAgenticSystemUseCase {
    #[must_use]
    pub const fn new(
        publications: Arc<dyn AgenticSystemPublicationPort>,
        executions: Arc<dyn AgenticSystemExecutionStorePort>,
        launcher: Arc<CeremonyLauncher>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            publications,
            executions,
            launcher,
            bindings: None,
            clock,
        }
    }

    /// Record where the integrator wants to be reached, when there is
    /// somewhere to record.
    #[must_use]
    pub fn with_integrator_bindings(mut self, bindings: Arc<dyn IntegratorBindingPort>) -> Self {
        self.bindings = Some(bindings);
        self
    }

    #[tracing::instrument(
        name = "instantiate_agentic_system",
        skip_all,
        fields(
            agentic_system_id = %input.system_id(),
            execution_id = %input.execution_id(),
        )
    )]
    pub async fn execute(
        &self,
        input: InstantiateAgenticSystemInput,
    ) -> Result<AgenticSystemExecution, DomainError> {
        let published = self
            .publications
            .published(input.system_id(), input.revision())
            .await?
            .ok_or(DomainError::NotFound {
                what: "published_agentic_system",
            })?;
        let system = published.system();
        let materialized = materialization::of(system, input.offers())?;
        let planned = self.open(&published, &materialized, &input).await?;
        let Some(mut execution) = planned else {
            return self.existing(&published, &input).await;
        };

        let blocking = system.blocking_dependencies();
        for ceremony in system.ceremonies().keys() {
            if blocking.get(ceremony).is_none_or(|waits| !waits.is_empty()) {
                continue;
            }
            let link = self
                .begin(&execution, system, ceremony, &materialized, &input)
                .await?;
            execution = execution.with_link(ceremony, link, self.clock.now())?;
        }
        if let Some(binding) = self.bind_integrator(system, &input).await? {
            execution = execution.with_integrator_binding(binding, self.clock.now());
        }
        self.store(execution).await
    }

    /// Open the run, or report that it is already open.
    async fn open(
        &self,
        published: &PublishedAgenticSystem,
        materialized: &BTreeMap<
            made_core::value_objects::ParticipantId,
            ParticipantMaterialization,
        >,
        input: &InstantiateAgenticSystemInput,
    ) -> Result<Option<AgenticSystemExecution>, DomainError> {
        let system = published.system();
        let links = system.ceremonies().iter().map(|(ceremony, composition)| {
            (
                ceremony.clone(),
                CeremonyExecutionLink::pending(composition.pin().clone()),
            )
        });
        let planned = AgenticSystemExecution::plan(
            input.execution_id().clone(),
            published.pin(),
            links,
            system.profiles().clone(),
            materialized.clone(),
            self.clock.now(),
        )?;
        match self.executions.create(planned).await? {
            AgenticSystemExecutionCreation::Created(execution) => Ok(Some(execution)),
            AgenticSystemExecutionCreation::AlreadyExists(_) => Ok(None),
        }
    }

    /// The run that identity already names, once it is shown to be
    /// running the same design.
    ///
    /// The pin is compared rather than trusted: the same execution
    /// identity offered against a different revision is a caller
    /// mistake that would otherwise be answered with somebody else's
    /// run.
    async fn existing(
        &self,
        published: &PublishedAgenticSystem,
        input: &InstantiateAgenticSystemInput,
    ) -> Result<AgenticSystemExecution, DomainError> {
        let existing =
            self.executions
                .get(input.execution_id())
                .await?
                .ok_or(DomainError::NotFound {
                    what: "agentic_system_execution",
                })?;
        if existing.system().matches(&published.pin()) {
            return Ok(existing);
        }
        Err(DomainError::Conflict {
            what: "agentic_system_execution.system",
        })
    }

    /// Start one composition, or record why it was skipped.
    async fn begin(
        &self,
        execution: &AgenticSystemExecution,
        system: &AgenticSystem,
        ceremony: &SystemCeremonyId,
        materialized: &BTreeMap<
            made_core::value_objects::ParticipantId,
            ParticipantMaterialization,
        >,
        input: &InstantiateAgenticSystemInput,
    ) -> Result<CeremonyExecutionLink, DomainError> {
        let link = execution.link(ceremony).ok_or(DomainError::NotFound {
            what: "agentic_system_execution.ceremony",
        })?;
        if let Some(reason) = materialization::blocking(system, materialized, ceremony)? {
            return Ok(link.skipped(reason));
        }
        let composition = &system.ceremonies()[ceremony];
        let round = LoopRound::ZERO.next();
        let instance = self
            .launcher
            .launch(
                input.execution_id(),
                composition,
                round,
                input.inputs().get(ceremony).cloned().unwrap_or_default(),
                materialized,
                input.actor_id(),
                input.actor_kind(),
            )
            .await?;
        Ok(link.started(instance, round))
    }

    async fn bind_integrator(
        &self,
        system: &AgenticSystem,
        input: &InstantiateAgenticSystemInput,
    ) -> Result<Option<IntegratorBindingId>, DomainError> {
        let (Some(bindings), Some(destination)) =
            (self.bindings.as_ref(), input.integrator_destination())
        else {
            return Ok(None);
        };
        let id = IntegratorBindingId::new(format!("{}-integrator", input.execution_id()))?;
        let binding = IntegratorBinding::new(
            id.clone(),
            IntegratorScope::system_execution(input.execution_id().clone()),
            RoleId::new(system.integrator().as_str())?,
            destination.clone(),
            HostAgentIncarnation::new(input.execution_id().as_str())?,
            self.clock.now(),
        );
        bindings.bind(binding, BindReplacement::Refuse).await?;
        Ok(Some(id))
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
