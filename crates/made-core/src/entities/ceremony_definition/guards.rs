use std::collections::BTreeMap;

use crate::value_objects::{
    CeremonyContext, CeremonyGuard, CeremonyStep, CeremonyTransition, StateId, StepExecutionRecord,
    StepId,
};

use super::CeremonyDefinition;

impl CeremonyDefinition {
    #[must_use]
    pub fn guards_are_satisfied(
        &self,
        transition: &CeremonyTransition,
        records: &BTreeMap<StepId, StepExecutionRecord>,
        context: &CeremonyContext,
    ) -> bool {
        self.repeat_requirements_are_satisfied(transition.from(), records)
            && transition.required_guards().iter().all(|guard_name| {
                self.guards
                    .get(guard_name)
                    .is_some_and(|guard| self.guard_is_satisfied(guard, records, context))
            })
    }

    /// Evaluate one guard with definition-owned step repetition semantics.
    ///
    /// A raw `COMPLETED` record is not enough for a repeating step: its
    /// structured stop condition must also hold. Keeping that rule here makes
    /// transition selection, instance projections and aggregate enforcement
    /// agree on what completion means.
    #[must_use]
    pub fn guard_is_satisfied(
        &self,
        guard: &CeremonyGuard,
        records: &BTreeMap<StepId, StepExecutionRecord>,
        context: &CeremonyContext,
    ) -> bool {
        if !guard.is_satisfied(records, context) {
            return false;
        }
        match guard.condition() {
            crate::value_objects::GuardCondition::StepStatus { step_id, status }
                if status.is_success() =>
            {
                self.repeat_requirement_is_satisfied(step_id, records)
            }
            crate::value_objects::GuardCondition::AllStepsCompleted => self
                .steps
                .keys()
                .all(|step_id| self.repeat_requirement_is_satisfied(step_id, records)),
            _ => true,
        }
    }

    /// Whether every repeating step in `state_id` has reached its declared
    /// structured stop condition.
    #[must_use]
    pub fn repeat_requirements_are_satisfied(
        &self,
        state_id: &StateId,
        records: &BTreeMap<StepId, StepExecutionRecord>,
    ) -> bool {
        self.steps_for_state(state_id)
            .all(|step| self.repeat_requirement_is_satisfied(step.id(), records))
    }

    fn repeat_requirement_is_satisfied(
        &self,
        step_id: &StepId,
        records: &BTreeMap<StepId, StepExecutionRecord>,
    ) -> bool {
        let Some(policy) = self.step(step_id).and_then(CeremonyStep::repeat_policy) else {
            return true;
        };
        records.get(step_id).is_some_and(|record| {
            record.status().is_success() && policy.is_satisfied(record.output())
        })
    }
}
