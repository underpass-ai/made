//! Durable execution receipt inspection and fenced application.

use std::sync::Arc;

use made_adapters::grpc::MadeGrpcServiceBuilder;
use made_app::artifacts::ArtifactService;
use made_app::budgets::BudgetLedgerService;
use made_app::services::SessionStream;
use made_app::usecases::ResolveCeremonyDefinitionUseCase;
use made_app::workers::{
    CompleteExecutionReceiptUseCase, GetExecutionReceiptUseCase, InspectExecutionRecoveryUseCase,
};
use made_core::ports::{ClockPort, ExecutionReceiptStorePort};

pub(super) fn wire(
    builder: MadeGrpcServiceBuilder,
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    receipts: Arc<dyn ExecutionReceiptStorePort>,
    clock: Arc<dyn ClockPort>,
    artifacts: Option<Arc<ArtifactService>>,
    budgets: BudgetLedgerService,
) -> MadeGrpcServiceBuilder {
    let builder = builder
        .get_execution_receipt(Arc::new(GetExecutionReceiptUseCase::new(receipts.clone())))
        .inspect_execution_recovery(Arc::new(InspectExecutionRecoveryUseCase::new(
            stream.clone(),
            receipts.clone(),
        )));
    let complete = CompleteExecutionReceiptUseCase::new(definitions, stream, receipts, clock);
    let complete = if let Some(artifacts) = artifacts {
        complete.with_artifacts(artifacts)
    } else {
        complete
    };
    let complete = complete.with_budget_ledger(budgets);
    builder.complete_execution_receipt(Arc::new(complete))
}
