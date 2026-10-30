use serde::{Deserialize, Serialize};

use super::{ClaimDispositionKind, StepClaimFence, StepId};

/// What a plan says about one outstanding claim, against the exact
/// claim it observed.
///
/// The fence is carried so a plan cannot dispose of a claim that has
/// since been replaced. A stale fence fails the plan rather than being
/// ignored: the whole point of naming the claim is that the answer
/// belongs to that claim and not to whatever holds the step now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimDisposition {
    step_id: StepId,
    claim_fence: StepClaimFence,
    kind: ClaimDispositionKind,
}

impl ClaimDisposition {
    #[must_use]
    pub const fn new(
        step_id: StepId,
        claim_fence: StepClaimFence,
        kind: ClaimDispositionKind,
    ) -> Self {
        Self {
            step_id,
            claim_fence,
            kind,
        }
    }

    #[must_use]
    pub const fn step_id(&self) -> &StepId {
        &self.step_id
    }

    #[must_use]
    pub const fn claim_fence(&self) -> &StepClaimFence {
        &self.claim_fence
    }

    #[must_use]
    pub const fn kind(&self) -> &ClaimDispositionKind {
        &self.kind
    }
}
