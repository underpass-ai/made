use std::sync::Arc;

use made_core::entities::ceremony_commands::ApplyExecutionReceiptResult;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::{ClockPort, ExecutionReceiptStorePort};
use made_core::value_objects::{ExecutionReceiptLink, ExecutionReceiptLinkKind};

use super::CompleteExecutionReceiptInput;
use crate::services::{session_facts, ConflictPolicy, SessionStream};
use crate::usecases::ResolveCeremonyDefinitionUseCase;

/// Consume a persisted receipt and its result in one fenced ceremony append.
pub struct CompleteExecutionReceiptUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    receipts: Arc<dyn ExecutionReceiptStorePort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for CompleteExecutionReceiptUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompleteExecutionReceiptUseCase")
            .finish_non_exhaustive()
    }
}

impl CompleteExecutionReceiptUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        stream: Arc<SessionStream>,
        receipts: Arc<dyn ExecutionReceiptStorePort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            definitions,
            stream,
            receipts,
            clock,
        }
    }

    pub async fn execute(
        &self,
        input: CompleteExecutionReceiptInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let operation =
            self.receipts
                .operation(&input.operation_id)
                .await?
                .ok_or(DomainError::NotFound {
                    what: "execution_operation",
                })?;
        let receipt =
            self.receipts
                .receipt(&input.operation_id)
                .await?
                .ok_or(DomainError::NotFound {
                    what: "execution_receipt",
                })?;
        if operation.ceremony_id() != &input.ceremony_id
            || operation.step_id() != &input.step_id
            || receipt.operation_id() != &input.operation_id
            || receipt.request_digest() != operation.request_digest()
        {
            return Err(DomainError::InvariantViolated {
                reason: "execution receipt does not belong to the requested ceremony step",
            });
        }

        let actual_link_kind = if receipt.producer_claim_fence() == &input.claim_fence {
            ExecutionReceiptLinkKind::Direct
        } else {
            if !receipt.recovery_capability().supports_automatic_recovery() {
                return Err(DomainError::InvariantViolated {
                    reason: "execution receipt requires reconciliation before adoption",
                });
            }
            ExecutionReceiptLinkKind::Adopted
        };
        if actual_link_kind != input.link_kind {
            return Err(DomainError::Conflict {
                what: "execution_receipt_link_kind",
            });
        }
        let link = ExecutionReceiptLink::new(
            receipt.receipt_id().clone(),
            receipt.operation_id().clone(),
            receipt.producer_claim_fence().clone(),
            input.claim_fence.clone(),
            actual_link_kind,
        )?;
        let session = self.stream.load(&input.ceremony_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let now = self.clock.now();
        let command = CeremonyCommand::ApplyExecutionReceiptResult(ApplyExecutionReceiptResult {
            step_id: input.step_id,
            claim_fence: input.claim_fence,
            receipt_link: link,
            result: receipt.result().clone(),
            now,
        });
        self.stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&command, &definition)?;
                if events.is_empty() {
                    return Ok(Vec::new());
                }
                let actor = session_facts::step_result_seat(&events, input.actor_kind)?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await
            .map(|session| session.instance)
    }
}
