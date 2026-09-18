use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{CeremonyId, ChildCompletionRef, ChildSpawnPlan, StepClaimFence};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildGroupState {
    plan: ChildSpawnPlan,
    adopted_claim_fence: StepClaimFence,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    completions: BTreeMap<CeremonyId, ChildCompletionRef>,
}

impl ChildGroupState {
    #[must_use]
    pub fn new(plan: ChildSpawnPlan) -> Self {
        let adopted_claim_fence = plan.active_claim_fence().clone();
        Self {
            plan,
            adopted_claim_fence,
            completions: BTreeMap::new(),
        }
    }
    #[must_use]
    pub fn plan(&self) -> &ChildSpawnPlan {
        &self.plan
    }
    #[must_use]
    pub fn adopted_claim_fence(&self) -> &StepClaimFence {
        &self.adopted_claim_fence
    }
    #[must_use]
    pub fn completions(&self) -> &BTreeMap<CeremonyId, ChildCompletionRef> {
        &self.completions
    }
    pub(crate) fn adopt(&mut self, fence: StepClaimFence) {
        self.adopted_claim_fence = fence;
    }
    pub(crate) fn accept(&mut self, completion: ChildCompletionRef) {
        self.completions
            .insert(completion.child_id().clone(), completion);
    }
}
