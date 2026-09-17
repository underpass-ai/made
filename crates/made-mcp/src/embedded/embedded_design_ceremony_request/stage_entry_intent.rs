use made_app::usecases::{
    CeremonyDesignGroup, CeremonyDesignGroupStep, CeremonyDesignJoin, CeremonyDesignStageEntry,
};
use made_core::error::DomainError;
use made_core::value_objects::{JoinStepCount, StateExecution, StepId};
use serde::{de::Error as _, Deserialize, Deserializer};
use serde_json::Value;

use super::StageIntent;

#[derive(Clone, Debug)]
pub(super) enum StageEntryIntent {
    Group(GroupStageIntent),
    Leaf(StageIntent),
}

impl<'de> Deserialize<'de> for StageEntryIntent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if value
            .as_object()
            .is_some_and(|object| object.contains_key("group"))
        {
            serde_json::from_value(value)
                .map(Self::Group)
                .map_err(D::Error::custom)
        } else {
            serde_json::from_value(value)
                .map(Self::Leaf)
                .map_err(D::Error::custom)
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GroupStageIntent {
    id: String,
    group: GroupIntent,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupIntent {
    #[serde(default)]
    execution: Option<String>,
    steps: Vec<StageIntent>,
    #[serde(default)]
    join: Option<JoinIntent>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JoinIntent {
    condition: String,
    #[serde(default)]
    count: Option<u32>,
}

impl StageEntryIntent {
    pub(super) fn into_domain(self) -> Result<CeremonyDesignStageEntry, DomainError> {
        match self {
            Self::Leaf(stage) => stage.into_domain().map(CeremonyDesignStageEntry::Leaf),
            Self::Group(stage) => {
                let execution = match stage.group.execution.as_deref() {
                    None | Some("sequential") => StateExecution::Sequential,
                    Some("concurrent") => StateExecution::Concurrent,
                    Some(_) => {
                        return Err(DomainError::InvalidDocument {
                            reason: "group execution must be `sequential` or `concurrent`"
                                .to_owned(),
                        })
                    }
                };
                let join = match stage.group.join {
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
                let steps = stage
                    .group
                    .steps
                    .into_iter()
                    .map(|step| step.into_domain().map(CeremonyDesignGroupStep::new))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(CeremonyDesignStageEntry::Group(CeremonyDesignGroup::new(
                    StepId::new(stage.id)?,
                    execution,
                    steps,
                    join,
                )))
            }
        }
    }
}
