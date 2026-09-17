use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{CeremonyContext, GuardCondition, GuardName, StepExecutionRecord, StepId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyGuard {
    name: GuardName,
    condition: GuardCondition,
}

impl CeremonyGuard {
    #[must_use]
    pub fn new(name: GuardName, condition: GuardCondition) -> Self {
        Self { name, condition }
    }

    #[must_use]
    pub fn name(&self) -> &GuardName {
        &self.name
    }

    #[must_use]
    pub fn condition(&self) -> &GuardCondition {
        &self.condition
    }

    #[must_use]
    pub fn is_satisfied(
        &self,
        records: &BTreeMap<StepId, StepExecutionRecord>,
        context: &CeremonyContext,
    ) -> bool {
        match &self.condition {
            GuardCondition::Always => true,
            GuardCondition::AllStepsCompleted => {
                !records.is_empty() && records.values().all(|record| record.status().is_success())
            }
            GuardCondition::StepStatus { step_id, status } => records
                .get(step_id)
                .is_some_and(|record| record.status() == *status),
            GuardCondition::OutputField(condition) => records
                .get(condition.step_id())
                .is_some_and(|record| condition.is_satisfied(record)),
            // Exhaustion needs the referenced step policy and the transition
            // source. CeremonyDefinition evaluates it with both available.
            GuardCondition::StepRepeatExhausted(_) => false,
            GuardCondition::HumanApproval => context.is_guard_approved(&self.name),
        }
    }
}
