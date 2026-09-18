use made_core::error::DomainError;
use made_core::value_objects::{CeremonyTimeout, DurationMs, StateTimeout, StepTimeout};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct CeremonyTimeoutsDocument {
    #[serde(default)]
    step_default: Option<u64>,
    #[serde(default)]
    ceremony: Option<u64>,
    #[serde(default)]
    state_default: Option<u64>,
}

impl CeremonyTimeoutsDocument {
    pub(super) fn default_step_timeout(&self) -> Result<Option<StepTimeout>, DomainError> {
        self.step_default
            .filter(|seconds| *seconds > 0)
            .map(|seconds| StepTimeout::new(DurationMs::from_millis(seconds.saturating_mul(1000))))
            .transpose()
    }

    pub(super) fn ceremony_timeout(&self) -> Result<Option<CeremonyTimeout>, DomainError> {
        self.ceremony
            .filter(|seconds| *seconds > 0)
            .map(|seconds| {
                CeremonyTimeout::new(DurationMs::from_millis(seconds.saturating_mul(1000)))
            })
            .transpose()
    }

    pub(super) fn state_timeout(&self) -> Result<Option<StateTimeout>, DomainError> {
        self.state_default
            .filter(|seconds| *seconds > 0)
            .map(|seconds| StateTimeout::new(DurationMs::from_millis(seconds.saturating_mul(1000))))
            .transpose()
    }
}
