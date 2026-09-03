use std::sync::Arc;

use made_adapters::config::ServiceConfig;
use made_adapters::nats::NatsTriggerSubscriber;
use made_core::ports::{
    AgentRegistryPort, AgentResolverPort, ContractRegistryPort, CouncilRegistryPort,
    DeliberationRepositoryPort,
};

/// Aggregate of every handle the composition root produces.
pub struct Application {
    pub service_config: ServiceConfig,
    pub agent_registry: Arc<dyn AgentRegistryPort>,
    pub agent_resolver: Arc<dyn AgentResolverPort>,
    pub council_registry: Arc<dyn CouncilRegistryPort>,
    pub contract_registry: Arc<dyn ContractRegistryPort>,
    pub repository: Arc<dyn DeliberationRepositoryPort>,
    pub grpc_service: made_adapters::grpc::MadeGrpcService,
    pub nats_subscriber: Option<NatsTriggerSubscriber>,
    pub health_state: crate::health::HealthState,
}

impl std::fmt::Debug for Application {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Application")
            .field("service_config", &self.service_config)
            .field("nats_subscriber_enabled", &self.nats_subscriber.is_some())
            .finish()
    }
}
