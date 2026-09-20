//! Where designs, seals and runs are held, for the service.
//!
//! The same decision the ceremony store makes, made once more here and
//! from the same configuration. A service that kept its designs in
//! memory would restart having forgotten the revision its running
//! systems are pinned to, and every one of those runs would become
//! unexplainable.
//!
//! Its own module rather than a block in the composition root: the
//! root has one job, a fourth wiring block would push it past the
//! budget that keeps it readable, and the choice made here — durable
//! or not, and why — is worth a page of its own.

use std::sync::Arc;

use made_adapters::config::ServiceConfig;
use made_adapters::grpc::{AgenticSystemOperations, MadeGrpcServiceBuilder};
use made_adapters::memory::ForgetfulMemory;
use made_adapters::memory::{
    InMemoryAgenticSystemExecutions, InMemoryAgenticSystemPublications,
    InMemoryAgenticSystemRepository,
};
use made_adapters::mermaid::AgenticSystemMermaidDiagram;
use made_adapters::postgres::{
    PostgresAgenticSystemExecutions, PostgresAgenticSystemPublications,
    PostgresAgenticSystemRepository, PostgresPool,
};
use made_adapters::sqlite::{
    SqliteAgenticSystemExecutions, SqliteAgenticSystemPublications, SqliteAgenticSystemRepository,
};
use made_app::services::SessionStream;
use made_app::usecases::agentic_system::{CeremonyLauncher, Observation};
use made_app::usecases::{
    BindCeremonyParticipantsUseCase, ResolveCeremonyDefinitionUseCase,
    StartPublishedCeremonyUseCase,
};
use made_core::ports::{
    AgenticSystemDiagramPort, CeremonyDefinitionPublicationPort, CeremonyDefinitionRepositoryPort,
    ClockPort, IntegratorBindingPort,
};
use tracing::{info, warn};

use crate::{AgenticSystemHandles, ComposeError};

/// Pick the agentic-system adapters that match the configured store.
pub(super) fn wire(
    config: &ServiceConfig,
    postgres: Option<&PostgresPool>,
) -> Result<AgenticSystemHandles, ComposeError> {
    let diagrams: Arc<dyn AgenticSystemDiagramPort> = Arc::new(AgenticSystemMermaidDiagram::new());
    if let Some(pool) = postgres {
        info!("agentic system designs, seals and runs are durable in Postgres");
        return Ok(AgenticSystemHandles {
            repository: Arc::new(PostgresAgenticSystemRepository::new(pool.clone())),
            publications: Arc::new(PostgresAgenticSystemPublications::new(pool.clone())),
            executions: Arc::new(PostgresAgenticSystemExecutions::new(pool.clone())),
            diagrams,
        });
    }
    let Some(path) = config.ceremony_store_path.as_deref() else {
        warn!(
            "MADE_CEREMONY_STORE_PATH is unset: agentic system designs are held in memory and \
             will not survive a restart"
        );
        return Ok(AgenticSystemHandles {
            repository: Arc::new(InMemoryAgenticSystemRepository::new()),
            publications: Arc::new(InMemoryAgenticSystemPublications::new()),
            executions: Arc::new(InMemoryAgenticSystemExecutions::new()),
            diagrams,
        });
    };
    let repository = SqliteAgenticSystemRepository::open(path).map_err(|error| {
        ComposeError::CeremonyStore(format!("agentic system repository at {path}: {error}"))
    })?;
    let publications = SqliteAgenticSystemPublications::open(path).map_err(|error| {
        ComposeError::CeremonyStore(format!("agentic system publications at {path}: {error}"))
    })?;
    let executions = SqliteAgenticSystemExecutions::open(path).map_err(|error| {
        ComposeError::CeremonyStore(format!("agentic system executions at {path}: {error}"))
    })?;
    info!(path, "agentic system designs, seals and runs are durable");
    Ok(AgenticSystemHandles {
        repository: Arc::new(repository),
        publications: Arc::new(publications),
        executions: Arc::new(executions),
        diagrams,
    })
}

/// What the agentic-system operations need beyond their own stores.
///
/// A named bundle rather than six positional arguments: the
/// composition root already has six things called `ceremony_*`, and a
/// call that mixed two of them would still compile.
pub(super) struct AgenticSystemDependencies {
    pub(super) publications: Arc<dyn CeremonyDefinitionPublicationPort>,
    pub(super) definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
    pub(super) stream: Arc<SessionStream>,
    pub(super) clock: Arc<dyn ClockPort>,
    pub(super) bindings: Arc<dyn IntegratorBindingPort>,
}

/// Hand the service the operations, built from the stores and the
/// use cases a host would call itself.
///
/// Starting a composed ceremony goes through `start_published` and
/// `bind_participants`, the same two a host uses by hand, so a system
/// run and a hand-started ceremony leave the same records behind.
pub(super) fn apply_to(
    builder: MadeGrpcServiceBuilder,
    handles: &AgenticSystemHandles,
    deps: AgenticSystemDependencies,
) -> MadeGrpcServiceBuilder {
    let launcher = Arc::new(CeremonyLauncher::new(
        Arc::new(StartPublishedCeremonyUseCase::new(
            deps.publications.clone(),
            deps.stream.clone(),
            deps.clock.clone(),
            Arc::new(ForgetfulMemory::new()),
        )),
        Arc::new(BindCeremonyParticipantsUseCase::new(
            Arc::new(ResolveCeremonyDefinitionUseCase::new(
                deps.definitions,
                deps.publications.clone(),
            )),
            deps.stream.clone(),
            deps.clock.clone(),
        )),
    ));
    builder.agentic_system(Arc::new(AgenticSystemOperations::new(
        handles.repository.clone(),
        handles.publications.clone(),
        handles.executions.clone(),
        handles.diagrams.clone(),
        deps.publications,
        launcher,
        Arc::new(Observation::new(deps.stream)),
        Some(deps.bindings),
        deps.clock,
    )))
}
