use made_app::usecases::CeremonyDesignStepRepeatExhaustedGuard;
use made_core::error::DomainError;
use made_core::value_objects::StepId;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StepRepeatExhaustedGuardIntent {
    step: String,
}

impl StepRepeatExhaustedGuardIntent {
    pub(super) fn into_domain(self) -> Result<CeremonyDesignStepRepeatExhaustedGuard, DomainError> {
        Ok(CeremonyDesignStepRepeatExhaustedGuard::new(StepId::new(
            self.step,
        )?))
    }
}
