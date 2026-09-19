use super::ReceiptStatus;
use made_core::value_objects::{ExternalOperationId, StepClaimFence};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptObservation {
    producer_claim_fence: StepClaimFence,
    external_operation_id: Option<ExternalOperationId>,
    status: ReceiptStatus,
    stdout_bytes: u64,
    stderr_bytes: u64,
    observed_at: OffsetDateTime,
}

impl ScriptObservation {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        producer_claim_fence: StepClaimFence,
        external_operation_id: Option<ExternalOperationId>,
        status: ReceiptStatus,
        stdout_bytes: u64,
        stderr_bytes: u64,
        observed_at: OffsetDateTime,
    ) -> Self {
        Self {
            producer_claim_fence,
            external_operation_id,
            status,
            stdout_bytes,
            stderr_bytes,
            observed_at,
        }
    }
    #[must_use]
    pub const fn producer_claim_fence(&self) -> &StepClaimFence {
        &self.producer_claim_fence
    }
    #[must_use]
    pub const fn external_operation_id(&self) -> Option<&ExternalOperationId> {
        self.external_operation_id.as_ref()
    }
    #[must_use]
    pub const fn status(&self) -> ReceiptStatus {
        self.status
    }
    #[must_use]
    pub const fn stdout_bytes(&self) -> u64 {
        self.stdout_bytes
    }
    #[must_use]
    pub const fn stderr_bytes(&self) -> u64 {
        self.stderr_bytes
    }
    #[must_use]
    pub const fn observed_at(&self) -> OffsetDateTime {
        self.observed_at
    }
}
