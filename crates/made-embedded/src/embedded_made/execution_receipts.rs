//! In-process parity for durable execution receipt operations.

use made_app::workers::{
    CompleteExecutionReceiptInput, CompleteExecutionReceiptUseCase, ExecutionRecoveryItemsPage,
    GetExecutionReceiptUseCase, InspectExecutionRecoveryUseCase,
};
use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::value_objects::{
    ExecutionOperationId, ExecutionReceipt, ExecutionReceiptLinkKind, ExecutionRecoveryCursor,
    ExecutionRecoveryPageLimit,
};

use super::EmbeddedMade;

impl EmbeddedMade {
    pub async fn get_execution_receipt(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<ExecutionReceipt, DomainError> {
        GetExecutionReceiptUseCase::new(self.execution_receipts.clone())
            .execute(operation_id)
            .await
    }

    pub async fn inspect_execution_recovery(
        &self,
        after: Option<&ExecutionRecoveryCursor>,
        limit: ExecutionRecoveryPageLimit,
    ) -> Result<ExecutionRecoveryItemsPage, DomainError> {
        InspectExecutionRecoveryUseCase::new(self.stream.clone(), self.execution_receipts.clone())
            .execute(after, limit)
            .await
    }

    pub async fn complete_execution_receipt(
        &self,
        mut input: CompleteExecutionReceiptInput,
    ) -> Result<CeremonyInstance, DomainError> {
        input.link_kind = ExecutionReceiptLinkKind::Direct;
        CompleteExecutionReceiptUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.execution_receipts.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn adopt_execution_receipt(
        &self,
        mut input: CompleteExecutionReceiptInput,
    ) -> Result<CeremonyInstance, DomainError> {
        input.link_kind = ExecutionReceiptLinkKind::Adopted;
        CompleteExecutionReceiptUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.execution_receipts.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }
}
