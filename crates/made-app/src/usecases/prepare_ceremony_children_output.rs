use made_core::entities::CeremonyInstance;
use made_core::value_objects::{CeremonyId, ChildGroupId, StepResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareCeremonyChildrenOutput {
    instance: CeremonyInstance,
    group_id: ChildGroupId,
    child_ids: Vec<CeremonyId>,
    result: StepResult,
}

impl PrepareCeremonyChildrenOutput {
    #[must_use]
    pub fn new(
        instance: CeremonyInstance,
        group_id: ChildGroupId,
        child_ids: Vec<CeremonyId>,
        result: StepResult,
    ) -> Self {
        Self {
            instance,
            group_id,
            child_ids,
            result,
        }
    }
    #[must_use]
    pub fn instance(&self) -> &CeremonyInstance {
        &self.instance
    }
    #[must_use]
    pub fn group_id(&self) -> &ChildGroupId {
        &self.group_id
    }
    #[must_use]
    pub fn child_ids(&self) -> &[CeremonyId] {
        &self.child_ids
    }
    #[must_use]
    pub fn result(&self) -> &StepResult {
        &self.result
    }
    #[must_use]
    pub fn into_parts(self) -> (CeremonyInstance, StepResult) {
        (self.instance, self.result)
    }
}
