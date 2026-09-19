//! Persistence adapter selection for the service composition root.

use std::sync::Arc;

use made_adapters::config::ServiceConfig;
use made_adapters::memory::{
    InMemoryAgentRegistry, InMemoryContractRegistry, InMemoryCouncilJournal,
    InMemoryCouncilRegistry, InMemoryDeliberationRepository, InMemoryStatistics,
};
use made_adapters::postgres::{
    PostgresAgentRegistry, PostgresConfig, PostgresContractRegistry, PostgresCouncilJournal,
    PostgresCouncilRegistry, PostgresDeliberationRepository, PostgresPool, PostgresStatistics,
};
use made_adapters::sqlite::{
    SqliteAgentRegistry, SqliteContractRegistry, SqliteCouncilJournal, SqliteCouncilRegistry,
    SqliteCouncilStatistics, SqliteCouncilStore, SqliteDeliberationRepository,
};
use made_core::ports::AgentFactoryPort;
use tracing::info;

use super::Persistence;
use crate::ComposeError;

/// Pick persistent backings based on config. When `MADE_POSTGRES_URL`
/// is set, every registry that has a Postgres adapter goes through
/// it; migrations apply on startup so a fresh cluster is exercisable.
/// Otherwise the configured local store holds all council state, or explicit in-memory defaults are wired when no durable store is configured.
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
            contract_registry: Arc::new(PostgresContractRegistry::new(pool.clone())),
            council_journal: Arc::new(PostgresCouncilJournal::new(pool.clone())),
            pool: Some(pool),
        })
    } else if let Some(path) = cfg.ceremony_store_path.as_deref() {
        let store = SqliteCouncilStore::open(path)?;
        let agents = Arc::new(SqliteAgentRegistry::new(store.clone(), agent_factory));
        info!(path, "councils, agents, contracts, deliberations, statistics and council journal are durable in SQLite");
        Ok(Persistence {
            repository: Arc::new(SqliteDeliberationRepository::new(store.clone())),
            council_registry: Arc::new(SqliteCouncilRegistry::new(store.clone())),
            agent_registry: agents.clone(),
            agent_resolver: agents,
            statistics: Arc::new(SqliteCouncilStatistics::new(store.clone())),
            contract_registry: Arc::new(SqliteContractRegistry::new(store.clone())),
            council_journal: Arc::new(SqliteCouncilJournal::new(store)),
            pool: None,
        })
    } else {
        info!("no durable council backend configured; using ephemeral in-memory persistence");
        let agents = Arc::new(InMemoryAgentRegistry::new());
        Ok(Persistence {
            repository: Arc::new(InMemoryDeliberationRepository::new()),
            council_registry: Arc::new(InMemoryCouncilRegistry::new()),
            agent_registry: agents.clone(),
            agent_resolver: agents,
            statistics: Arc::new(InMemoryStatistics::new()),
            contract_registry: Arc::new(InMemoryContractRegistry::new()),
            council_journal: Arc::new(InMemoryCouncilJournal::new()),
            pool: None,
        })
    }
}

#[cfg(test)]
#[path = "persistence_tests.rs"]
mod tests;
