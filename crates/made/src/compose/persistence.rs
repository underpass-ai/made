//! Persistence adapter selection for the service composition root.

use std::sync::Arc;

use made_adapters::config::ServiceConfig;
use made_adapters::memory::{
    InMemoryAgentRegistry, InMemoryCouncilRegistry, InMemoryDeliberationRepository,
    InMemoryStatistics,
};
use made_adapters::postgres::{
    PostgresAgentRegistry, PostgresConfig, PostgresCouncilRegistry, PostgresDeliberationRepository,
    PostgresPool, PostgresStatistics,
};
use made_core::ports::AgentFactoryPort;
use tracing::info;

use super::Persistence;
use crate::ComposeError;

/// Pick persistent backings based on config. When `MADE_POSTGRES_URL`
/// is set, every registry that has a Postgres adapter goes through
/// it; migrations apply on startup so a fresh cluster is exercisable.
/// Otherwise the in-memory defaults are wired.
pub(super) async fn wire_persistence(
    cfg: &ServiceConfig,
    agent_factory: Arc<dyn AgentFactoryPort>,
) -> Result<Persistence, ComposeError> {
    if let Some(url) = cfg.postgres_url.as_deref() {
        let pool = PostgresPool::connect(&PostgresConfig::from_url(url)).await?;
        pool.run_migrations().await?;
        let agents = Arc::new(PostgresAgentRegistry::new(pool.clone(), agent_factory));
        info!("postgres persistence wired (deliberations, councils, agents, statistics)");
        Ok(Persistence {
            repository: Arc::new(PostgresDeliberationRepository::new(pool.clone())),
            council_registry: Arc::new(PostgresCouncilRegistry::new(pool.clone())),
            agent_registry: agents.clone(),
            agent_resolver: agents,
            statistics: Arc::new(PostgresStatistics::new(pool.clone())),
            pool: Some(pool),
        })
    } else {
        info!("postgres disabled; using in-memory persistence");
        let agents = Arc::new(InMemoryAgentRegistry::new());
        Ok(Persistence {
            repository: Arc::new(InMemoryDeliberationRepository::new()),
            council_registry: Arc::new(InMemoryCouncilRegistry::new()),
            agent_registry: agents.clone(),
            agent_resolver: agents,
            statistics: Arc::new(InMemoryStatistics::new()),
            pool: None,
        })
    }
}
