use serde::{Deserialize, Serialize};

use super::{ExecutionConnectorId, ExecutionIntent, ExecutionOperationId, ExecutionRequestDigest};
use crate::value_objects::StepClaimFence;

/// Durable evidence that a connector actually reported an unresolved effect.
///
/// This state is separate from the connector's immutable recovery capability:
/// a queryable connector can still exhaust recovery and become ambiguous.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReconciliationRequirement {
    operation_id: ExecutionOperationId,
    request_digest: ExecutionRequestDigest,
    producer_claim_fence: StepClaimFence,
    connector_id: ExecutionConnectorId,
}

impl ExecutionReconciliationRequirement {
    #[must_use]
    pub fn from_intent(intent: &ExecutionIntent) -> Self {
        Self {
            operation_id: intent.operation().operation_id().clone(),
            request_digest: intent.operation().request_digest().clone(),
            producer_claim_fence: intent.claim_fence().clone(),
            connector_id: intent.connector_id().clone(),
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
    pub const fn producer_claim_fence(&self) -> &StepClaimFence {
        &self.producer_claim_fence
    }

    #[must_use]
    pub const fn connector_id(&self) -> &ExecutionConnectorId {
        &self.connector_id
    }

    #[must_use]
    pub fn matches_intent(&self, intent: &ExecutionIntent) -> bool {
        self == &Self::from_intent(intent)
    }
}
