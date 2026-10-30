//! In-process parity for durable execution receipt operations.

use made_app::workers::{
    CompleteExecutionReceiptInput, CompleteExecutionReceiptUseCase, ExecutionRecoveryItemsPage,
    GetExecutionReceiptUseCase, InspectExecutionRecoveryUseCase,
};
use made_core::entities::ProtocolInstance;
use made_core::error::DomainError;
use made_core::value_objects::{
    AuthorizationAction, ExecutionOperationId, ExecutionReceipt, ExecutionReceiptLinkKind,
    ExecutionRecoveryCursor, ExecutionRecoveryPageLimit,
};

use super::EmbeddedMade;

impl EmbeddedMade {
    pub async fn get_execution_receipt(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<ExecutionReceipt, DomainError> {
        self.require_authorized_action(AuthorizationAction::GetExecutionReceipt)?;
        let operation = self
            .execution_receipts
            .operation(operation_id)
            .await?
            .ok_or(DomainError::NotFound {
                what: "execution_operation",
            })?;
        self.require_authorized_protocol_action(
            AuthorizationAction::GetExecutionReceipt,
            operation.protocol_id(),
        )?;
        GetExecutionReceiptUseCase::new(self.execution_receipts.clone())
            .execute(operation_id)
            .await
    }

    pub async fn inspect_execution_recovery(
        &self,
        after: Option<&ExecutionRecoveryCursor>,
        limit: ExecutionRecoveryPageLimit,
    ) -> Result<ExecutionRecoveryItemsPage, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::InspectExecutionRecovery)?;
        InspectExecutionRecoveryUseCase::new(self.stream.clone(), self.execution_receipts.clone())
            .execute(after, limit)
            .await
    }

    pub async fn complete_execution_receipt(
        &self,
        mut input: CompleteExecutionReceiptInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::CompleteExecutionReceipt,
            &input.protocol_id,
        )?;
        input.link_kind = ExecutionReceiptLinkKind::Direct;
        let complete = CompleteExecutionReceiptUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.execution_receipts.clone(),
            self.clock.clone(),
        );
        let complete = if let Some(artifacts) = &self.artifacts {
            complete.with_artifacts(artifacts.clone())
        } else {
            complete
        };
        let complete = complete.with_budget_ledger(self.budgets.clone());
        complete.execute(input).await
    }

    pub async fn adopt_execution_receipt(
        &self,
        mut input: CompleteExecutionReceiptInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::AdoptExecutionReceipt,
            &input.protocol_id,
        )?;
        input.link_kind = ExecutionReceiptLinkKind::Adopted;
        let complete = CompleteExecutionReceiptUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.execution_receipts.clone(),
            self.clock.clone(),
        );
        let complete = if let Some(artifacts) = &self.artifacts {
            complete.with_artifacts(artifacts.clone())
        } else {
            complete
        };
        let complete = complete.with_budget_ledger(self.budgets.clone());
        complete.execute(input).await
    }
}
