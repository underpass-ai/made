use made_core::value_objects::{
    ExecutionIntent, ExecutionOperationId, ExecutionRequestDigest, StepClaimFence,
};
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct ScriptInvocation {
    operation_id: ExecutionOperationId,
    request_digest: ExecutionRequestDigest,
    claim_fence: StepClaimFence,
    request_bytes: Vec<u8>,
}

impl ScriptInvocation {
    #[must_use]
    pub fn from_intent(intent: &ExecutionIntent) -> Self {
        Self {
            operation_id: intent.operation().operation_id().clone(),
            request_digest: intent.operation().request_digest().clone(),
            claim_fence: intent.claim_fence().clone(),
            request_bytes: intent.operation().request().as_bytes().to_vec(),
        }
    }
    #[must_use]
    pub const fn operation_id(&self) -> &ExecutionOperationId {
        &self.operation_id
    }
    #[must_use]
    pub const fn request_digest(&self) -> &ExecutionRequestDigest {
        &self.request_digest
    }
    #[must_use]
    pub const fn claim_fence(&self) -> &StepClaimFence {
        &self.claim_fence
    }
    #[must_use]
    pub fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }
}

impl fmt::Debug for ScriptInvocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScriptInvocation")
            .field("operation_id", &self.operation_id)
            .field("request_digest", &self.request_digest)
            .field("claim_fence", &self.claim_fence)
            .field("request_byte_count", &self.request_bytes.len())
            .finish()
    }
}
