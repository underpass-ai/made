use std::sync::Arc;

use made_adapters::grpc::MadeGrpcServiceBuilder;
use made_app::budgets::{
    BudgetLedgerService, BudgetedStepClaimUseCase, StartBudgetedProtocolUseCase,
};
use made_app::services::SessionStream;
use made_app::usecases::ResolveProtocolDefinitionUseCase;
use made_core::ports::{
    BudgetLedgerStorePort, ClockPort, MemoryReaderPort, ProtocolDefinitionPublicationPort,
};
use made_core::value_objects::MaxParallel;

/// Budget services composed once and shared by admission, reporting and receipt reconciliation.
pub(super) struct BudgetOperations {
    service: BudgetLedgerService,
    start: Arc<StartBudgetedProtocolUseCase>,
    claim: Arc<BudgetedStepClaimUseCase>,
}

impl BudgetOperations {
    pub(super) fn new(
        store: Arc<dyn BudgetLedgerStorePort>,
        publications: Arc<dyn ProtocolDefinitionPublicationPort>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
        memory: Arc<dyn MemoryReaderPort>,
        definitions: Arc<ResolveProtocolDefinitionUseCase>,
        max_parallel: MaxParallel,
    ) -> Self {
        let service = BudgetLedgerService::new(store, clock.clone());
        let start = Arc::new(StartBudgetedProtocolUseCase::new(
            publications,
            stream.clone(),
            clock.clone(),
            memory,
            service.clone(),
        ));
        let claim = Arc::new(
            BudgetedStepClaimUseCase::new(definitions, stream, clock, service.clone())
                .with_max_parallel_ceiling(max_parallel),
        );
        Self {
            service,
            start,
            claim,
        }
    }

    pub(super) fn wire(&self, builder: MadeGrpcServiceBuilder) -> MadeGrpcServiceBuilder {
        builder
            .start_budgeted_protocol(self.start.clone())
            .budgeted_step_claim(self.claim.clone())
            .budgets(Arc::new(self.service.clone()))
    }

    pub(super) fn service(&self) -> BudgetLedgerService {
        self.service.clone()
    }

    pub(super) fn claim(&self) -> Arc<BudgetedStepClaimUseCase> {
        self.claim.clone()
    }
}
