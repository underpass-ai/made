use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    ExecutionReceiptStorePort, ExecutionRecoveryPage, RecordExecutionIntentOutcome,
    RecordExecutionReceiptOutcome,
};
use made_core::value_objects::{
    ExecutionIntent, ExecutionOperation, ExecutionOperationId, ExecutionReceipt,
    ExecutionRecoveryCursor, ExecutionRecoveryPageLimit, StepClaimFence,
};
use sqlx::Row;

use super::ceremony_store::{decode, encode, sqlx_error};
use super::PostgresCeremonyStore;

fn decode_operation(
    operation_id: &ExecutionOperationId,
    payload: &[u8],
) -> Result<ExecutionOperation, DomainError> {
    let operation: ExecutionOperation = decode(payload, "decode execution operation")?;
    operation.validate()?;
    if operation.operation_id() != operation_id {
        return Err(DomainError::InvariantViolated {
            reason: "postgres: execution operation key does not match its payload",
        });
    }
    Ok(operation)
}

fn decode_intent(
    operation_id: &ExecutionOperationId,
    claim_fence: Option<&StepClaimFence>,
    payload: &[u8],
) -> Result<ExecutionIntent, DomainError> {
    let intent: ExecutionIntent = decode(payload, "decode execution intent")?;
    intent.validate()?;
    if intent.operation().operation_id() != operation_id
        || claim_fence.is_some_and(|fence| intent.claim_fence() != fence)
    {
        return Err(DomainError::InvariantViolated {
            reason: "postgres: execution intent key does not match its payload",
        });
    }
    Ok(intent)
}

fn decode_receipt(
    operation_id: &ExecutionOperationId,
    payload: &[u8],
) -> Result<ExecutionReceipt, DomainError> {
    let receipt: ExecutionReceipt = decode(payload, "decode execution receipt")?;
    receipt.validate()?;
    if receipt.operation_id() != operation_id {
        return Err(DomainError::InvariantViolated {
            reason: "postgres: execution receipt key does not match its payload",
        });
    }
    Ok(receipt)
}

#[async_trait]
impl ExecutionReceiptStorePort for PostgresCeremonyStore {
    async fn record_intent(
        &self,
        intent: ExecutionIntent,
    ) -> Result<RecordExecutionIntentOutcome, DomainError> {
        intent.validate()?;
        let operation_id = intent.operation().operation_id();
        let operation_payload = encode(intent.operation(), "encode execution operation")?;
        let intent_payload = encode(&intent, "encode execution intent")?;
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin execution intent"))?;
        let inserted = sqlx::query(
            "INSERT INTO ceremony_execution_operations (operation_id, payload) \
             VALUES ($1, $2) ON CONFLICT (operation_id) DO NOTHING",
        )
        .bind(operation_id.as_str())
        .bind(operation_payload)
        .execute(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "insert execution operation"))?;
        let operation_row = sqlx::query(
            "SELECT payload FROM ceremony_execution_operations \
             WHERE operation_id = $1 FOR UPDATE",
        )
        .bind(operation_id.as_str())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "lock execution operation"))?;
        let stored_operation_payload: Vec<u8> = operation_row
            .try_get("payload")
            .map_err(|error| sqlx_error(error, "decode execution operation payload"))?;
        let stored_operation = decode_operation(operation_id, &stored_operation_payload)?;
        if &stored_operation != intent.operation() {
            return Err(DomainError::Conflict {
                what: "execution_operation",
            });
        }
        let existing = sqlx::query(
            "SELECT payload FROM ceremony_execution_intents \
             WHERE operation_id = $1 AND claim_fence = $2",
        )
        .bind(operation_id.as_str())
        .bind(intent.claim_fence().as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "read execution intent"))?;
        if let Some(existing) = existing {
            let payload: Vec<u8> = existing
                .try_get("payload")
                .map_err(|error| sqlx_error(error, "decode execution intent payload"))?;
            let stored = decode_intent(operation_id, Some(intent.claim_fence()), &payload)?;
            return if stored == intent {
                Ok(RecordExecutionIntentOutcome::AlreadyRecorded)
            } else {
                Err(DomainError::Conflict {
                    what: "execution_intent",
                })
            };
        }
        sqlx::query(
            "INSERT INTO ceremony_execution_intents \
             (operation_id, claim_fence, recorded_at, payload) VALUES ($1, $2, $3, $4)",
        )
        .bind(operation_id.as_str())
        .bind(intent.claim_fence().as_str())
        .bind(intent.recorded_at())
        .bind(intent_payload)
        .execute(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "insert execution intent"))?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit execution intent"))?;
        if inserted.rows_affected() == 1 {
            Ok(RecordExecutionIntentOutcome::RecordedFirst)
        } else {
            Ok(RecordExecutionIntentOutcome::RecordedAdditional)
        }
    }

    async fn intent(
        &self,
        operation_id: &ExecutionOperationId,
        claim_fence: &StepClaimFence,
    ) -> Result<Option<ExecutionIntent>, DomainError> {
        let row = sqlx::query(
            "SELECT payload FROM ceremony_execution_intents \
             WHERE operation_id = $1 AND claim_fence = $2",
        )
        .bind(operation_id.as_str())
        .bind(claim_fence.as_str())
        .fetch_optional(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "read execution intent"))?;
        row.map(|row| {
            let payload: Vec<u8> = row
                .try_get("payload")
                .map_err(|error| sqlx_error(error, "decode execution intent payload"))?;
            decode_intent(operation_id, Some(claim_fence), &payload)
        })
        .transpose()
    }

    async fn receipt(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Option<ExecutionReceipt>, DomainError> {
        let row =
            sqlx::query("SELECT payload FROM ceremony_execution_receipts WHERE operation_id = $1")
                .bind(operation_id.as_str())
                .fetch_optional(self.pool.inner())
                .await
                .map_err(|error| sqlx_error(error, "read execution receipt"))?;
        row.map(|row| {
            let payload: Vec<u8> = row
                .try_get("payload")
                .map_err(|error| sqlx_error(error, "decode execution receipt payload"))?;
            decode_receipt(operation_id, &payload)
        })
        .transpose()
    }

    async fn intents(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Vec<ExecutionIntent>, DomainError> {
        let rows = sqlx::query(
            "SELECT claim_fence, payload FROM ceremony_execution_intents \
             WHERE operation_id = $1 ORDER BY claim_fence",
        )
        .bind(operation_id.as_str())
        .fetch_all(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "list execution intents"))?;
        rows.into_iter()
            .map(|row| {
                let claim_fence: String = row
                    .try_get("claim_fence")
                    .map_err(|error| sqlx_error(error, "decode execution claim fence"))?;
                let claim_fence = StepClaimFence::new(claim_fence)?;
                let payload: Vec<u8> = row
                    .try_get("payload")
                    .map_err(|error| sqlx_error(error, "decode execution intent payload"))?;
                decode_intent(operation_id, Some(&claim_fence), &payload)
            })
            .collect()
    }

    async fn operation(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Option<ExecutionOperation>, DomainError> {
        let row = sqlx::query(
            "SELECT payload FROM ceremony_execution_operations WHERE operation_id = $1",
        )
        .bind(operation_id.as_str())
        .fetch_optional(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "read execution operation"))?;
        row.map(|row| {
            let payload: Vec<u8> = row
                .try_get("payload")
                .map_err(|error| sqlx_error(error, "decode execution operation payload"))?;
            decode_operation(operation_id, &payload)
        })
        .transpose()
    }

    async fn record_receipt(
        &self,
        receipt: ExecutionReceipt,
    ) -> Result<RecordExecutionReceiptOutcome, DomainError> {
        receipt.validate()?;
        let operation_id = receipt.operation_id();
        let payload = encode(&receipt, "encode execution receipt")?;
        let mut transaction = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin execution receipt"))?;
        let operation_row = sqlx::query(
            "SELECT payload FROM ceremony_execution_operations \
             WHERE operation_id = $1 FOR UPDATE",
        )
        .bind(operation_id.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "lock execution operation for receipt"))?;
        let Some(operation_row) = operation_row else {
            return Err(DomainError::NotFound {
                what: "execution_operation",
            });
        };
        let operation_payload: Vec<u8> = operation_row
            .try_get("payload")
            .map_err(|error| sqlx_error(error, "decode execution operation payload"))?;
        let operation = decode_operation(operation_id, &operation_payload)?;
        let intent_row = sqlx::query(
            "SELECT payload FROM ceremony_execution_intents \
             WHERE operation_id = $1 AND claim_fence = $2",
        )
        .bind(operation_id.as_str())
        .bind(receipt.producer_claim_fence().as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "read receipt producer intent"))?;
        let Some(intent_row) = intent_row else {
            return Err(DomainError::InvariantViolated {
                reason: "execution receipt does not match a recorded intent",
            });
        };
        let intent_payload: Vec<u8> = intent_row
            .try_get("payload")
            .map_err(|error| sqlx_error(error, "decode execution intent payload"))?;
        let intent = decode_intent(
            operation_id,
            Some(receipt.producer_claim_fence()),
            &intent_payload,
        )?;
        if operation.request_digest() != receipt.request_digest()
            || intent.connector_id() != receipt.connector_id()
            || intent.recovery_capability() != receipt.recovery_capability()
            || intent.source_kind() != receipt.source_kind()
        {
            return Err(DomainError::InvariantViolated {
                reason: "execution receipt does not match its producer intent contract",
            });
        }
        let existing =
            sqlx::query("SELECT payload FROM ceremony_execution_receipts WHERE operation_id = $1")
                .bind(operation_id.as_str())
                .fetch_optional(&mut *transaction)
                .await
                .map_err(|error| sqlx_error(error, "read existing execution receipt"))?;
        if let Some(existing) = existing {
            let existing_payload: Vec<u8> = existing
                .try_get("payload")
                .map_err(|error| sqlx_error(error, "decode execution receipt payload"))?;
            let stored = decode_receipt(operation_id, &existing_payload)?;
            return if stored == receipt {
                Ok(RecordExecutionReceiptOutcome::AlreadyRecorded)
            } else {
                Err(DomainError::Conflict {
                    what: "execution_receipt",
                })
            };
        }
        sqlx::query(
            "INSERT INTO ceremony_execution_receipts \
             (operation_id, receipt_id, observed_at, payload) VALUES ($1, $2, $3, $4)",
        )
        .bind(operation_id.as_str())
        .bind(receipt.receipt_id().as_str())
        .bind(receipt.observed_at())
        .bind(payload)
        .execute(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "insert execution receipt"))?;
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit execution receipt"))?;
        Ok(RecordExecutionReceiptOutcome::Recorded)
    }

    async fn recoverable(
        &self,
        after: Option<&ExecutionRecoveryCursor>,
        limit: ExecutionRecoveryPageLimit,
    ) -> Result<ExecutionRecoveryPage, DomainError> {
        let rows = sqlx::query(
            "SELECT operation_id, payload FROM ceremony_execution_operations \
             WHERE operation_id > $1 ORDER BY operation_id LIMIT $2",
        )
        .bind(after.map_or("", ExecutionRecoveryCursor::as_str))
        .bind(i64::from(limit.get()) + 1)
        .fetch_all(self.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "scan recoverable executions"))?;
        let wanted = usize::from(limit.get());
        let has_more = rows.len() > wanted;
        let operations = rows
            .into_iter()
            .take(wanted)
            .map(|row| {
                let operation_id: String = row
                    .try_get("operation_id")
                    .map_err(|error| sqlx_error(error, "decode execution operation id"))?;
                let operation_id = ExecutionOperationId::new(operation_id)?;
                let payload: Vec<u8> = row
                    .try_get("payload")
                    .map_err(|error| sqlx_error(error, "decode execution operation payload"))?;
                decode_operation(&operation_id, &payload)
            })
            .collect::<Result<Vec<_>, DomainError>>()?;
        let next_cursor = if has_more {
            operations
                .last()
                .map(|operation| ExecutionRecoveryCursor::new(operation.operation_id().as_str()))
                .transpose()?
        } else {
            None
        };
        Ok(ExecutionRecoveryPage::new(operations, next_cursor))
    }
}
