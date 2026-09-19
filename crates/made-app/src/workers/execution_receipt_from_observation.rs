use made_core::error::DomainError;
use made_core::ports::{
    CeremonyExecutionConnectorPort, CeremonyExecutionObservation, ExecutionReceiptStorePort,
};
use made_core::value_objects::{ExecutionIntent, ExecutionReceipt};

pub(super) async fn execution_receipt_from_observation(
    store: &dyn ExecutionReceiptStorePort,
    connector: &dyn CeremonyExecutionConnectorPort,
    intent: &ExecutionIntent,
    observation: CeremonyExecutionObservation,
) -> Result<ExecutionReceipt, DomainError> {
    let producer_intent = store
        .intent(
            intent.operation().operation_id(),
            observation.producer_claim_fence(),
        )
        .await?
        .ok_or(DomainError::InvariantViolated {
            reason: "execution observation names an unknown producer claim fence",
        })?;
    if producer_intent.connector_id() != connector.connector_id()
        || producer_intent.recovery_capability() != connector.recovery_capability()
        || producer_intent.source_kind() != connector.source_kind()
    {
        return Err(DomainError::InvariantViolated {
            reason: "execution observation does not match the producer connector contract",
        });
    }
    let (_, external_operation_id, result, artifacts, observed_at) = observation.into_parts();
    ExecutionReceipt::new(
        intent.operation().operation_id().clone(),
        intent.operation().request_digest().clone(),
        producer_intent.claim_fence().clone(),
        producer_intent.connector_id().clone(),
        external_operation_id,
        producer_intent.recovery_capability(),
        producer_intent.source_kind(),
        result,
        artifacts,
        observed_at,
    )
}
