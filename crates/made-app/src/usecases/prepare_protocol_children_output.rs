use made_core::entities::ProtocolInstance;
use made_core::value_objects::{ChildGroupId, ProtocolId, StepResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareProtocolChildrenOutput {
    instance: ProtocolInstance,
    group_id: ChildGroupId,
    child_ids: Vec<ProtocolId>,
    result: StepResult,
}

impl PrepareProtocolChildrenOutput {
    #[must_use]
    pub fn new(
        instance: ProtocolInstance,
        group_id: ChildGroupId,
        child_ids: Vec<ProtocolId>,
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
    pub fn instance(&self) -> &ProtocolInstance {
        &self.instance
    }
    #[must_use]
    pub fn group_id(&self) -> &ChildGroupId {
        &self.group_id
    }
    #[must_use]
    pub fn child_ids(&self) -> &[ProtocolId] {
        &self.child_ids
    }
    #[must_use]
    pub fn result(&self) -> &StepResult {
        &self.result
    }
    #[must_use]
    pub fn into_parts(self) -> (ProtocolInstance, StepResult) {
        (self.instance, self.result)
    }
}
