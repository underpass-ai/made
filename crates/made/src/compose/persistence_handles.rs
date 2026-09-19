use made_adapters::postgres::PostgresPool;
use made_core::ports::{
    AgentRegistryPort, AgentResolverPort, ContractRegistryPort, CouncilJournalPort,
    CouncilRegistryPort, DeliberationRepositoryPort, StatisticsPort,
};
use std::sync::Arc;
/// Persistent handles selected together so one deployment never splits
/// its source of truth across storage backends.
pub(super) struct Persistence {
    pub(super) repository: Arc<dyn DeliberationRepositoryPort>,
    pub(super) council_registry: Arc<dyn CouncilRegistryPort>,
    pub(super) agent_registry: Arc<dyn AgentRegistryPort>,
    pub(super) agent_resolver: Arc<dyn AgentResolverPort>,
    pub(super) statistics: Arc<dyn StatisticsPort>,
    pub(super) contract_registry: Arc<dyn ContractRegistryPort>,
    pub(super) council_journal: Arc<dyn CouncilJournalPort>,
    pub(super) pool: Option<PostgresPool>,
}
