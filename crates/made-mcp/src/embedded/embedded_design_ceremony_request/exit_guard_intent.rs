use made_app::usecases::CeremonyDesignExitGuard;
use made_core::error::DomainError;
use made_core::value_objects::{ChildJoin, ChildQuorum, ChildrenCompletedCondition, StepId};
use serde::Deserialize;

use super::{OutputFieldGuardIntent, StepRepeatExhaustedGuardIntent};

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum ExitGuardIntent {
    OutputField(OutputFieldGuardIntent),
    StepRepeatExhausted(StepRepeatExhaustedGuardIntent),
    ChildrenCompleted {
        step: String,
        join: String,
        #[serde(default)]
        count: Option<u64>,
    },
}

impl ExitGuardIntent {
    pub(super) fn into_domain(self) -> Result<CeremonyDesignExitGuard, DomainError> {
        match self {
            Self::OutputField(guard) => {
                Ok(CeremonyDesignExitGuard::OutputField(guard.into_domain()?))
            }
            Self::StepRepeatExhausted(guard) => Ok(CeremonyDesignExitGuard::StepRepeatExhausted(
                guard.into_domain()?,
            )),
            Self::ChildrenCompleted { step, join, count } => {
                let join = match (join.as_str(), count) {
                    ("all", None) => ChildJoin::All,
                    ("any", None) => ChildJoin::Any,
                    ("quorum", Some(count)) => ChildJoin::Quorum {
                        count: ChildQuorum::new(u16::try_from(count).unwrap_or(u16::MAX))?,
                    },
                    _ => {
                        return Err(DomainError::InvalidDocument {
                            reason: "invalid children_completed join or count".to_owned(),
                        });
                    }
                };
                Ok(CeremonyDesignExitGuard::ChildrenCompleted(
                    ChildrenCompletedCondition::new(StepId::new(step)?, join),
                ))
            }
        }
    }
}
