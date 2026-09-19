//! Durable execution receipt inspection and fenced application.

use std::sync::Arc;

use made_adapters::grpc::MadeGrpcServiceBuilder;
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
) -> MadeGrpcServiceBuilder {
    builder
        .get_execution_receipt(Arc::new(GetExecutionReceiptUseCase::new(receipts.clone())))
        .inspect_execution_recovery(Arc::new(InspectExecutionRecoveryUseCase::new(
            stream.clone(),
            receipts.clone(),
        )))
        .complete_execution_receipt(Arc::new(CompleteExecutionReceiptUseCase::new(
            definitions,
            stream,
            receipts,
            clock,
        )))
}
