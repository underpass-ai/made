//! Everything a deliberation reaches for.

use std::sync::Arc;

use made_core::ports::{
    AgentResolverPort, ClockPort, ContractRegistryPort, CouncilRegistryPort,
    DeliberationRepositoryPort, ExecutorPort, MessagingPort, MetricsRecorderPort, ScoringPort,
    StatisticsPort, ValidatorPort,
};

/// The adapters one deliberation is built from.
///
/// A named bundle rather than eleven positional arguments: several of
/// them are `Arc<dyn …>` over traits a call site could mix up and
/// still compile.
pub(super) struct CouncilDependencies {
    pub(super) clock: Arc<dyn ClockPort>,
    pub(super) councils: Arc<dyn CouncilRegistryPort>,
    pub(super) resolver: Arc<dyn AgentResolverPort>,
    pub(super) validators: Vec<Arc<dyn ValidatorPort>>,
    pub(super) scoring: Arc<dyn ScoringPort>,
    pub(super) repository: Arc<dyn DeliberationRepositoryPort>,
    pub(super) messaging: Arc<dyn MessagingPort>,
    pub(super) statistics: Arc<dyn StatisticsPort>,
    pub(super) metrics: Arc<dyn MetricsRecorderPort>,
    pub(super) executor: Arc<dyn ExecutorPort>,
    pub(super) contracts: Arc<dyn ContractRegistryPort>,
}
