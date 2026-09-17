use made_app::usecases::CeremonyDesignExitGuard;
use made_core::error::DomainError;
use serde::Deserialize;

use super::{OutputFieldGuardIntent, StepRepeatExhaustedGuardIntent};

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum ExitGuardIntent {
    OutputField(OutputFieldGuardIntent),
    StepRepeatExhausted(StepRepeatExhaustedGuardIntent),
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
        }
    }
}
