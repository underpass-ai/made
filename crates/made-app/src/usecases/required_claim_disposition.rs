use made_core::value_objects::{CeremonyClaimPhase, StepClaimFence, StepId};
use serde::{Deserialize, Serialize};

/// One outstanding claim a handoff has to answer for, named by the
/// exact claim the report saw.
///
/// The fence travels because the answer belongs to this claim and not
/// to whatever holds the step by the time somebody replies. A plan
/// built from a stale report is refused rather than applied to the
/// wrong claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredClaimDisposition {
    pub step_id: StepId,
    pub claim_fence: StepClaimFence,
    pub phase: CeremonyClaimPhase,
}
