//! Everything the council family is built from.
//!
//! One struct rather than eleven positional arguments: the list is the
//! composition, and two ports of the same shape swapped by accident is
//! a defect nothing else would catch.

use std::sync::Arc;

use made_core::ports::{
    AgentResolverPort, ClockPort, ContractRegistryPort, CouncilRegistryPort,
    DeliberationRepositoryPort, ExecutorPort, MessagingPort, MetricsRecorderPort, ScoringPort,
    StatisticsPort, ValidatorPort,
};

/// What the council family is built from.
pub(super) struct CouncilPorts {
    pub(super) clock: Arc<dyn ClockPort>,
    pub(super) council_registry: Arc<dyn CouncilRegistryPort>,
    pub(super) contract_registry: Arc<dyn ContractRegistryPort>,
    pub(super) agent_resolver: Arc<dyn AgentResolverPort>,
    pub(super) validators: Vec<Arc<dyn ValidatorPort>>,
    pub(super) scoring: Arc<dyn ScoringPort>,
    pub(super) repository: Arc<dyn DeliberationRepositoryPort>,
    pub(super) messaging: Arc<dyn MessagingPort>,
    pub(super) statistics: Arc<dyn StatisticsPort>,
    pub(super) metrics: Arc<dyn MetricsRecorderPort>,
    pub(super) executor: Arc<dyn ExecutorPort>,
}
