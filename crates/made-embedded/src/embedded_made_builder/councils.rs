//! Council composition for the embedded builder.
use super::EmbeddedMadeBuilder;
use crate::{
    embedded_council_services::EmbeddedCouncilServices, unconfigured_executor::UnconfiguredExecutor,
};
use made_adapters::memory::{
    InMemoryAgentRegistry, InMemoryContractRegistry, InMemoryCouncilJournal,
    InMemoryCouncilRegistry, InMemoryDeliberationRepository,
};
use made_adapters::scoring::UniformScoring;
use made_adapters::validators::{
    AllowedStringValuesValidator, BoundedEventShapeValidator, ClaimsEvidenceGroundedValidator,
    ClaimsEvidenceSupportedValidator, ContentNonEmptyValidator, JsonObjectOutputValidator,
    JsonSchemaValidator, RequiredFieldsValidator,
};
use made_core::ports::{
    AgentFactoryPort, AgentRegistryPort, AgentResolverPort, ClockPort, ContractRegistryPort,
    CouncilRegistryPort, DeliberationRepositoryPort, ExecutorPort, MetricsRecorderPort,
    ScoringPort, StatisticsPort, ValidatorPort,
};
use std::sync::Arc;
impl EmbeddedMadeBuilder {
    pub(super) fn compose_councils(
        &mut self,
        clock: Arc<dyn ClockPort>,
        statistics: Arc<dyn StatisticsPort>,
        metrics: Arc<dyn MetricsRecorderPort>,
    ) -> Arc<EmbeddedCouncilServices> {
        let councils = self.council_registry.take().unwrap_or_else(|| {
            Arc::new(InMemoryCouncilRegistry::new()) as Arc<dyn CouncilRegistryPort>
        });
        let (agent_registry, agent_resolver) = self
            .agent_registry
            .take()
            .zip(self.agent_resolver.take())
            .unwrap_or_else(|| {
                let registry = Arc::new(InMemoryAgentRegistry::new());
                (
                    registry.clone() as Arc<dyn AgentRegistryPort>,
                    registry as Arc<dyn AgentResolverPort>,
                )
            });
        let deliberations = self.deliberations.take().unwrap_or_else(|| {
            Arc::new(InMemoryDeliberationRepository::new()) as Arc<dyn DeliberationRepositoryPort>
        });
        let contracts = self.contracts.take().unwrap_or_else(|| {
            Arc::new(InMemoryContractRegistry::new()) as Arc<dyn ContractRegistryPort>
        });
        let journal = self
            .council_journal
            .take()
            .unwrap_or_else(|| Arc::new(InMemoryCouncilJournal::new()));
        let messaging =
            made_adapters::council_journal_messaging::CouncilJournalMessaging::new(journal.clone());
        let messaging = if let Some(transport) = self.messaging.take() {
            messaging.with_immediate_transport(transport)
        } else {
            messaging
        };
        let messaging = Arc::new(messaging);
        Arc::new(EmbeddedCouncilServices::new(
            clock,
            journal,
            councils,
            agent_registry,
            agent_resolver,
            self.agent_factory.take().unwrap_or_else(|| {
                Arc::new(made_adapters::agents::DispatchingAgentFactory::new())
                    as Arc<dyn AgentFactoryPort>
            }),
            deliberations,
            contracts,
            self.validators
                .take()
                .unwrap_or_else(default_council_validators),
            self.scoring
                .take()
                .unwrap_or_else(|| Arc::new(UniformScoring::new()) as Arc<dyn ScoringPort>),
            self.executor
                .take()
                .unwrap_or_else(|| Arc::new(UnconfiguredExecutor) as Arc<dyn ExecutorPort>),
            messaging,
            statistics,
            metrics,
        ))
    }
}

fn default_council_validators() -> Vec<Arc<dyn ValidatorPort>> {
    vec![
        Arc::new(ContentNonEmptyValidator::new()),
        Arc::new(JsonObjectOutputValidator::new()),
        Arc::new(RequiredFieldsValidator::new()),
        Arc::new(AllowedStringValuesValidator::new()),
        Arc::new(JsonSchemaValidator::new()),
        Arc::new(ClaimsEvidenceGroundedValidator::new()),
        Arc::new(ClaimsEvidenceSupportedValidator::new(None)),
        Arc::new(BoundedEventShapeValidator::new()),
    ]
}
