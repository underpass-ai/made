use choreo_core::error::DomainError;
use choreo_core::value_objects::{DurationMs, StepTimeout};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct CeremonyTimeoutsDocument {
    #[serde(default)]
    step_default: Option<u64>,
}

impl CeremonyTimeoutsDocument {
    pub(super) fn default_step_timeout(&self) -> Result<Option<StepTimeout>, DomainError> {
        self.step_default
            .filter(|seconds| *seconds > 0)
            .map(|seconds| StepTimeout::new(DurationMs::from_millis(seconds.saturating_mul(1000))))
            .transpose()
    }
}
