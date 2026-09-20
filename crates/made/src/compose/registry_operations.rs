use std::sync::Arc;

use made_adapters::grpc::MadeGrpcServiceBuilder;
use made_app::usecases::{
    CreateCouncilUseCase, DeleteCouncilUseCase, GetDeliberationUseCase, ListCouncilsUseCase,
    PrepareCeremonyParticipantsUseCase, RegisterAgentUseCase, UnregisterAgentUseCase,
};
use made_core::ports::{
    AgentFactoryPort, AgentRegistryPort, AgentResolverPort, ClockPort, ContractRegistryPort,
    CouncilRegistryPort, DeliberationRepositoryPort,
};

/// Registry operations share the same persistence and provider factory.
pub(super) struct RegistryOperations {
    pub(super) clock: Arc<dyn ClockPort>,
    pub(super) factory: Arc<dyn AgentFactoryPort>,
    pub(super) agents: Arc<dyn AgentRegistryPort>,
    pub(super) resolver: Arc<dyn AgentResolverPort>,
    pub(super) councils: Arc<dyn CouncilRegistryPort>,
    pub(super) repository: Arc<dyn DeliberationRepositoryPort>,
}

impl RegistryOperations {
    /// Apply the environment's seeds, so a freshly deployed service
    /// is exercisable rather than an empty one an operator has to
    /// populate by hand first.
    ///
    /// Here because the registries being seeded are exactly the ones
    /// this struct already holds. Contracts come last and separately:
    /// a contract names specialties, and seeding one before the
    /// agents that provide them would seed a promise nothing keeps.
    pub(super) async fn seed(
        &self,
        contracts: &dyn ContractRegistryPort,
    ) -> Result<(), crate::ComposeError> {
        crate::seeding::apply_env_seeding(
            self.clock.as_ref(),
            self.agents.as_ref(),
            self.councils.as_ref(),
        )
        .await?;
        crate::seeding::apply_contract_seeding(contracts).await?;
        Ok(())
    }

    pub(super) fn wire(self, builder: MadeGrpcServiceBuilder) -> MadeGrpcServiceBuilder {
        builder
            .create_council(Arc::new(CreateCouncilUseCase::new(
                self.clock.clone(),
                self.councils.clone(),
                self.resolver,
            )))
            .delete_council(Arc::new(DeleteCouncilUseCase::new(self.councils.clone())))
            .list_councils(Arc::new(ListCouncilsUseCase::new(self.councils.clone())))
            .get_deliberation(Arc::new(GetDeliberationUseCase::new(self.repository)))
            .register_agent(Arc::new(RegisterAgentUseCase::new(
                self.factory.clone(),
                self.agents.clone(),
            )))
            .unregister_agent(Arc::new(UnregisterAgentUseCase::new(self.agents.clone())))
            .prepare_ceremony_participants(Arc::new(PrepareCeremonyParticipantsUseCase::new(
                self.clock,
                self.factory,
                self.agents,
                self.councils,
            )))
    }
}
