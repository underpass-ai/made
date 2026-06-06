use choreo_core::error::DomainError;
use choreo_core::value_objects::{DurationMs, RetryPolicy, StepAttempt};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct RetryPolicyDocument {
    max_attempts: u32,
    #[serde(default)]
    backoff_seconds: u64,
}

impl RetryPolicyDocument {
    pub(super) fn into_domain(self) -> Result<RetryPolicy, DomainError> {
        Ok(RetryPolicy::new(
            StepAttempt::new(self.max_attempts)?,
            DurationMs::from_millis(self.backoff_seconds.saturating_mul(1000)),
        ))
    }
}
