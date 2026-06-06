use serde::{Deserialize, Serialize};

use super::{StepId, StepStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GuardCondition {
    Always,
    AllStepsCompleted,
    StepStatus { step_id: StepId, status: StepStatus },
    HumanApproval,
}

impl GuardCondition {
    #[must_use]
    pub fn referenced_step_id(&self) -> Option<&StepId> {
        match self {
            Self::StepStatus { step_id, .. } => Some(step_id),
            Self::Always | Self::AllStepsCompleted | Self::HumanApproval => None,
        }
    }
}
