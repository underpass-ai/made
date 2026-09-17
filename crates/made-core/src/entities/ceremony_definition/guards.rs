use std::collections::BTreeMap;

use crate::value_objects::{
    CeremonyContext, CeremonyGuard, CeremonyStep, CeremonyTransition, GuardCondition, StateId,
    StepExecutionRecord, StepId,
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
        self.repeat_requirements_are_satisfied_for_transition(transition, records)
            && transition.required_guards().iter().all(|guard_name| {
                self.guards.get(guard_name).is_some_and(|guard| {
                    self.guard_is_satisfied_for_transition(guard, transition, records, context)
                })
            })
    }

    /// Evaluate one guard with definition-owned step repetition semantics.
    ///
    /// A raw `COMPLETED` record is not enough for a repeating step: its
    /// structured stop condition must also hold. Keeping that rule here makes
    /// transition selection, instance projections and aggregate enforcement
    /// agree on what completion means.
    #[must_use]
    pub fn guard_is_satisfied_for_transition(
        &self,
        guard: &CeremonyGuard,
        transition: &CeremonyTransition,
        records: &BTreeMap<StepId, StepExecutionRecord>,
        context: &CeremonyContext,
    ) -> bool {
        match guard.condition() {
            GuardCondition::StepRepeatExhausted(condition) => self
                .step_repeat_is_exhausted_on_transition(transition, condition.step_id(), records),
            GuardCondition::StepStatus { step_id, status } if status.is_success() => {
                guard.is_satisfied(records, context)
                    && self.repeat_requirement_is_satisfied_or_waived(transition, step_id, records)
            }
            GuardCondition::AllStepsCompleted => {
                guard.is_satisfied(records, context)
                    && self.steps.keys().all(|step_id| {
                        self.repeat_requirement_is_satisfied_or_waived(transition, step_id, records)
                    })
            }
            GuardCondition::AnyStepCompleted => {
                self.steps_for_state(transition.from()).any(|step| {
                    records
                        .get(step.id())
                        .is_some_and(|record| record.status().is_success())
                })
            }
            GuardCondition::StepsCompleted(required) => {
                self.steps_for_state(transition.from())
                    .filter(|step| {
                        records
                            .get(step.id())
                            .is_some_and(|record| record.status().is_success())
                    })
                    .count()
                    >= required.get() as usize
            }
            _ => guard.is_satisfied(records, context),
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

    /// Whether source-state repeats either reached their stop condition or
    /// have an exact exhaustion guard on this transition.
    #[must_use]
    pub fn repeat_requirements_are_satisfied_for_transition(
        &self,
        transition: &CeremonyTransition,
        records: &BTreeMap<StepId, StepExecutionRecord>,
    ) -> bool {
        self.steps_for_state(transition.from()).all(|step| {
            self.repeat_requirement_is_satisfied_or_waived(transition, step.id(), records)
        })
    }

    fn repeat_requirement_is_satisfied_or_waived(
        &self,
        transition: &CeremonyTransition,
        step_id: &StepId,
        records: &BTreeMap<StepId, StepExecutionRecord>,
    ) -> bool {
        self.repeat_requirement_is_satisfied(step_id, records)
            || self.transition_waives_exhausted_repeat(transition, step_id, records)
    }

    fn transition_waives_exhausted_repeat(
        &self,
        transition: &CeremonyTransition,
        step_id: &StepId,
        records: &BTreeMap<StepId, StepExecutionRecord>,
    ) -> bool {
        transition.required_guards().iter().any(|guard_name| {
            self.guards.get(guard_name).is_some_and(|guard| {
                matches!(
                    guard.condition(),
                    GuardCondition::StepRepeatExhausted(condition)
                        if condition.step_id() == step_id
                            && self.step_repeat_is_exhausted_on_transition(
                                transition,
                                step_id,
                                records,
                            )
                )
            })
        })
    }

    fn step_repeat_is_exhausted_on_transition(
        &self,
        transition: &CeremonyTransition,
        step_id: &StepId,
        records: &BTreeMap<StepId, StepExecutionRecord>,
    ) -> bool {
        let Some(step) = self.step(step_id) else {
            return false;
        };
        if step.state_id() != transition.from() {
            return false;
        }
        let Some(policy) = step.repeat_policy() else {
            return false;
        };
        records.get(step_id).is_some_and(|record| {
            record.status().is_success()
                && !policy.is_satisfied(record.output())
                && !policy.permits_another_iteration(record.iteration())
        })
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
