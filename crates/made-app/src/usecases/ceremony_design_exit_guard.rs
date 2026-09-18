use made_core::value_objects::ChildrenCompletedCondition;

use super::{CeremonyDesignOutputFieldGuard, CeremonyDesignStepRepeatExhaustedGuard};

/// Additional condition conjoined with a designed stage's completion guard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeremonyDesignExitGuard {
    OutputField(CeremonyDesignOutputFieldGuard),
    StepRepeatExhausted(CeremonyDesignStepRepeatExhaustedGuard),
    ChildrenCompleted(ChildrenCompletedCondition),
}
