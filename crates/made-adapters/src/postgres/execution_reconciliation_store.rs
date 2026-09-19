use made_core::error::DomainError;
use made_core::value_objects::{
    ExecutionIntent, ExecutionOperationId, ExecutionReconciliationRequirement, StepClaimFence,
};
use sqlx::Row;

use super::ceremony_store::{decode, encode, sqlx_error};
use super::PostgresCeremonyStore;

/// Persists the durable observation that an execution outcome is ambiguous.
pub(super) struct PostgresExecutionReconciliationStore<'a> {
    store: &'a PostgresCeremonyStore,
}

impl<'a> PostgresExecutionReconciliationStore<'a> {
    pub(super) const fn new(store: &'a PostgresCeremonyStore) -> Self {
        Self { store }
    }

    pub(super) async fn record(
        &self,
        requirement: ExecutionReconciliationRequirement,
    ) -> Result<(), DomainError> {
        let operation_id = requirement.operation_id();
        let claim_fence = requirement.producer_claim_fence();
        let payload = encode(&requirement, "encode execution reconciliation requirement")?;
        let mut transaction = self
            .store
            .pool
            .inner()
            .begin()
            .await
            .map_err(|error| sqlx_error(error, "begin execution reconciliation requirement"))?;
        let intent_row = sqlx::query(
            "SELECT payload FROM ceremony_execution_intents \
             WHERE operation_id = $1 AND claim_fence = $2 FOR UPDATE",
        )
        .bind(operation_id.as_str())
        .bind(claim_fence.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "lock reconciliation producer intent"))?;
        let Some(intent_row) = intent_row else {
            return Err(DomainError::NotFound {
                what: "execution_intent",
            });
        };
        let intent_payload: Vec<u8> = intent_row
            .try_get("payload")
            .map_err(|error| sqlx_error(error, "decode reconciliation producer intent"))?;
        let intent = decode_intent(operation_id, claim_fence, &intent_payload)?;
        if !requirement.matches_intent(&intent) {
            return Err(DomainError::Conflict {
                what: "execution_reconciliation_requirement",
            });
        }
        sqlx::query(
            "INSERT INTO ceremony_execution_reconciliation_requirements \
             (operation_id, claim_fence, payload) VALUES ($1, $2, $3) \
             ON CONFLICT (operation_id, claim_fence) DO NOTHING",
        )
        .bind(operation_id.as_str())
        .bind(claim_fence.as_str())
        .bind(payload)
        .execute(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "insert execution reconciliation requirement"))?;
        let stored_row = sqlx::query(
            "SELECT payload FROM ceremony_execution_reconciliation_requirements \
             WHERE operation_id = $1 AND claim_fence = $2 FOR UPDATE",
        )
        .bind(operation_id.as_str())
        .bind(claim_fence.as_str())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| sqlx_error(error, "read execution reconciliation requirement"))?;
        let stored_payload: Vec<u8> = stored_row
            .try_get("payload")
            .map_err(|error| sqlx_error(error, "decode reconciliation requirement payload"))?;
        let stored = decode_requirement(operation_id, claim_fence, &stored_payload)?;
        if stored != requirement {
            return Err(DomainError::Conflict {
                what: "execution_reconciliation_requirement",
            });
        }
        transaction
            .commit()
            .await
            .map_err(|error| sqlx_error(error, "commit execution reconciliation requirement"))
    }

    pub(super) async fn get(
        &self,
        operation_id: &ExecutionOperationId,
        claim_fence: &StepClaimFence,
    ) -> Result<Option<ExecutionReconciliationRequirement>, DomainError> {
        let row = sqlx::query(
            "SELECT payload FROM ceremony_execution_reconciliation_requirements \
             WHERE operation_id = $1 AND claim_fence = $2",
        )
        .bind(operation_id.as_str())
        .bind(claim_fence.as_str())
        .fetch_optional(self.store.pool.inner())
        .await
        .map_err(|error| sqlx_error(error, "read execution reconciliation requirement"))?;
        row.map(|row| {
            let payload: Vec<u8> = row.try_get("payload").map_err(|error| {
                sqlx_error(error, "decode execution reconciliation requirement payload")
            })?;
            decode_requirement(operation_id, claim_fence, &payload)
        })
        .transpose()
    }
}

fn decode_intent(
    operation_id: &ExecutionOperationId,
    claim_fence: &StepClaimFence,
    payload: &[u8],
) -> Result<ExecutionIntent, DomainError> {
    let intent: ExecutionIntent = decode(payload, "decode execution intent")?;
    intent.validate()?;
    if intent.operation().operation_id() != operation_id || intent.claim_fence() != claim_fence {
        return Err(DomainError::InvariantViolated {
            reason: "postgres: execution intent key does not match its payload",
        });
    }
    Ok(intent)
}

fn decode_requirement(
    operation_id: &ExecutionOperationId,
    claim_fence: &StepClaimFence,
    payload: &[u8],
) -> Result<ExecutionReconciliationRequirement, DomainError> {
    let requirement: ExecutionReconciliationRequirement =
        decode(payload, "decode execution reconciliation requirement")?;
    if requirement.operation_id() != operation_id
        || requirement.producer_claim_fence() != claim_fence
    {
        return Err(DomainError::InvariantViolated {
            reason: "postgres: execution reconciliation requirement key does not match its payload",
        });
    }
    Ok(requirement)
}
