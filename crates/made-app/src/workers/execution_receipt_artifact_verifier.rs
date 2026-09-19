use made_core::error::DomainError;
use made_core::value_objects::ExecutionReceipt;

use crate::artifacts::ArtifactService;

pub(super) async fn verify_receipt_artifacts(
    artifacts: Option<&ArtifactService>,
    receipt: &ExecutionReceipt,
) -> Result<(), DomainError> {
    if receipt.artifacts().is_empty() {
        return Ok(());
    }
    artifacts
        .ok_or(DomainError::InvariantViolated {
            reason: "execution receipt carries artifacts but no artifact store is configured",
        })?
        .verify_execution_receipt(receipt)
        .await
}
