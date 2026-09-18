use made_app::usecases::{
    CeremonyDesignGroup, CeremonyDesignGroupRepeat, CeremonyDesignGroupRepeatUntil,
    CeremonyDesignGroupStep, CeremonyDesignJoin,
};
use made_core::error::DomainError;
use made_core::value_objects::{
    JoinStepCount, StateExecution, StateIteration, StepId, StepOutputField,
};
use serde::Deserialize;

use super::group_intent::GroupIntent;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::embedded::embedded_design_ceremony_request) struct GroupStageIntent {
    id: String,
    group: GroupIntent,
}

impl GroupStageIntent {
    pub(super) fn into_domain(self) -> Result<CeremonyDesignGroup, DomainError> {
        let execution = match self.group.execution.as_deref() {
            None | Some("sequential") => StateExecution::Sequential,
            Some("concurrent") => StateExecution::Concurrent,
            Some(_) => {
                return Err(DomainError::InvalidDocument {
                    reason: "group execution must be `sequential` or `concurrent`".to_owned(),
                })
            }
        };
        let join = match self.group.join {
            None => CeremonyDesignJoin::AllStepsCompleted,
            Some(join) => match (join.condition.as_str(), join.count) {
                ("all_steps_completed", None) => CeremonyDesignJoin::AllStepsCompleted,
                ("any_step_completed", None) => CeremonyDesignJoin::AnyStepCompleted,
                ("steps_completed", Some(count)) => {
                    CeremonyDesignJoin::StepsCompleted(JoinStepCount::new(count)?)
                }
                _ => {
                    return Err(DomainError::InvalidDocument {
                        reason: "invalid group join condition or count".to_owned(),
                    })
                }
            },
        };
        let steps = self
            .group
            .steps
            .into_iter()
            .map(|step| step.into_domain().map(CeremonyDesignGroupStep::new))
            .collect::<Result<Vec<_>, _>>()?;
        let group = CeremonyDesignGroup::new(StepId::new(self.id)?, execution, steps, join);
        Ok(match self.group.repeat {
            Some(repeat) => group.with_repeat(CeremonyDesignGroupRepeat::new(
                StateIteration::new(repeat.max_iterations)?,
                CeremonyDesignGroupRepeatUntil::new(
                    StepId::new(repeat.until.step)?,
                    StepOutputField::new(repeat.until.output_field)?,
                    repeat.until.equals,
                ),
            )),
            None => group,
        })
    }
}
