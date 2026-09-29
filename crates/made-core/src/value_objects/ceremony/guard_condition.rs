use serde::{Deserialize, Serialize};

use super::{
    ChildrenCompletedCondition, JoinStepCount, OutputFieldGuardCondition, StateId,
    StateRepeatExhaustedGuardCondition, StepId, StepRepeatExhaustedGuardCondition, StepStatus,
};

mod counted_join_condition;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GuardCondition {
    Always,
    AllStepsCompleted,
    AnyStepCompleted,
    StepsCompleted(#[serde(with = "counted_join_condition")] JoinStepCount),
    StepStatus { step_id: StepId, status: StepStatus },
    OutputField(OutputFieldGuardCondition),
    StepRepeatExhausted(StepRepeatExhaustedGuardCondition),
    ChildrenCompleted(ChildrenCompletedCondition),
    HumanApproval,
    StateRepeatExhausted(StateRepeatExhaustedGuardCondition),
}

impl GuardCondition {
    #[must_use]
    pub fn referenced_step_id(&self) -> Option<&StepId> {
        match self {
            Self::StepStatus { step_id, .. } => Some(step_id),
            Self::OutputField(condition) => Some(condition.step_id()),
            Self::StepRepeatExhausted(condition) => Some(condition.step_id()),
            Self::ChildrenCompleted(condition) => Some(condition.step_id()),
            Self::Always
            | Self::AllStepsCompleted
            | Self::AnyStepCompleted
            | Self::StepsCompleted(_)
            | Self::HumanApproval
            | Self::StateRepeatExhausted(_) => None,
        }
    }

    /// The state a state-level condition names, if it names one.
    #[must_use]
    pub fn referenced_state_id(&self) -> Option<&StateId> {
        match self {
            Self::StateRepeatExhausted(condition) => Some(condition.state_id()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counted_join_serializes_as_tagged_object() {
        let condition = GuardCondition::StepsCompleted(JoinStepCount::new(2).unwrap());
        let encoded = serde_json::to_string(&condition).unwrap();
        assert_eq!(encoded, r#"{"kind":"steps_completed","count":2}"#);
        assert_eq!(
            serde_json::from_str::<GuardCondition>(&encoded).unwrap(),
            condition
        );
    }

    #[test]
    fn counted_join_deserialization_preserves_positive_count_invariant() {
        for encoded in [
            r#"{"kind":"steps_completed","count":0}"#,
            r#"{"kind":"steps_completed","count":-1}"#,
            r#"{"kind":"steps_completed"}"#,
            r#"{"kind":"steps_completed","count":4294967296}"#,
        ] {
            assert!(
                serde_json::from_str::<GuardCondition>(encoded).is_err(),
                "{encoded}"
            );
        }
    }

    #[test]
    fn other_guard_variants_keep_their_exact_canonical_bytes() {
        use crate::value_objects::StepOutputField;
        let step = StepId::new("inspect_api").unwrap();
        let cases = [
            (GuardCondition::Always, r#"{"kind":"always"}"#),
            (
                GuardCondition::AllStepsCompleted,
                r#"{"kind":"all_steps_completed"}"#,
            ),
            (
                GuardCondition::AnyStepCompleted,
                r#"{"kind":"any_step_completed"}"#,
            ),
            (
                GuardCondition::HumanApproval,
                r#"{"kind":"human_approval"}"#,
            ),
            (
                GuardCondition::StepStatus {
                    step_id: step.clone(),
                    status: StepStatus::Completed,
                },
                r#"{"kind":"step_status","step_id":"inspect_api","status":"COMPLETED"}"#,
            ),
            (
                GuardCondition::OutputField(OutputFieldGuardCondition::new(
                    step.clone(),
                    StepOutputField::new("approved").unwrap(),
                    serde_json::json!(true),
                )),
                r#"{"kind":"output_field","step_id":"inspect_api","output_field":"approved","expected":true}"#,
            ),
            (
                GuardCondition::StepRepeatExhausted(StepRepeatExhaustedGuardCondition::new(step)),
                r#"{"kind":"step_repeat_exhausted","step_id":"inspect_api"}"#,
            ),
            (
                GuardCondition::StateRepeatExhausted(StateRepeatExhaustedGuardCondition::new(
                    StateId::new("REVIEW_CYCLE").unwrap(),
                )),
                r#"{"kind":"state_repeat_exhausted","state_id":"REVIEW_CYCLE"}"#,
            ),
        ];
        for (condition, encoded) in cases {
            assert_eq!(serde_json::to_string(&condition).unwrap(), encoded);
            assert_eq!(
                serde_json::from_str::<GuardCondition>(encoded).unwrap(),
                condition
            );
        }
    }
}
