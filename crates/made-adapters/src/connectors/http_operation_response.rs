use made_core::value_objects::{
    ExecutionIntent, ExecutionOperationId, ExecutionRequestDigest, ExternalOperationId,
    StepClaimFence, StepResult,
};
use made_core::{ports::CeremonyExecutionObservation, DomainError};
use serde::{Deserialize, Serialize};

/// Remote service protocol: durable identity and producer survive client receipt loss.
#[derive(Debug, Serialize, Deserialize)]
pub struct HttpOperationResponse {
    pub operation_id: ExecutionOperationId,
    pub request_digest: ExecutionRequestDigest,
    pub producer_claim_fence: StepClaimFence,
    pub result: StepResult,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: time::OffsetDateTime,
}

impl HttpOperationResponse {
    pub(crate) fn observe(
        self,
        intent: &ExecutionIntent,
    ) -> Result<CeremonyExecutionObservation, DomainError> {
        if &self.operation_id != intent.operation().operation_id()
            || &self.request_digest != intent.operation().request_digest()
        {
            return Err(DomainError::Conflict {
                what: "HTTP operation identity or digest",
            });
        }
        if &self.producer_claim_fence != intent.claim_fence() {
            return Err(DomainError::Conflict {
                what: "HTTP producer claim fence",
            });
        }
        Ok(CeremonyExecutionObservation::new(
            self.producer_claim_fence,
            Some(ExternalOperationId::new(format!(
                "http:{}",
                self.operation_id.as_str()
            ))?),
            self.result,
            Vec::new(),
            self.observed_at,
        ))
    }
}
