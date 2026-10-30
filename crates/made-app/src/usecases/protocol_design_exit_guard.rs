use made_core::value_objects::ChildrenCompletedCondition;

use super::{ProtocolDesignOutputFieldGuard, ProtocolDesignStepRepeatExhaustedGuard};

/// Additional condition conjoined with a designed stage's completion guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolDesignExitGuard {
    OutputField(ProtocolDesignOutputFieldGuard),
    StepRepeatExhausted(ProtocolDesignStepRepeatExhaustedGuard),
    ChildrenCompleted(ChildrenCompletedCondition),
}
