//! Deterministic provider adapter used by contract tests and local fixtures.

use super::operation::{
    FinishClassification, FinishReason, Observed, ProviderContractError,
    ProviderErrorClassification, ProviderIdentity, ProviderModel, ProviderName,
    ProviderOperationAdapter, ProviderOperationObservation, ProviderOperationRequest,
    ProviderSecret, Usage,
};
use made_core::value_objects::DurationMs;

/// A fixed response returned by [`FakeProviderAdapter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeProviderResponse {
    /// Successful provider declaration.
    Success {
        latency: DurationMs,
        usage: Observed<Usage>,
        finish: FinishReason,
    },
    /// Classified provider failure, still represented as an observation.
    Error {
        latency: DurationMs,
        usage: Observed<Usage>,
        error: ProviderErrorClassification,
    },
}

impl FakeProviderResponse {
    /// Build a success with fixture-declared usage.
    #[must_use]
    pub const fn success(latency_ms: u64, usage: Usage, finish: FinishReason) -> Self {
        Self::Success {
            latency: DurationMs::from_millis(latency_ms),
            usage: Observed::declared_by_fixture(usage),
            finish,
        }
    }

    /// Build an error with fixture-declared usage.
    #[must_use]
    pub const fn error(latency_ms: u64, usage: Usage, error: ProviderErrorClassification) -> Self {
        Self::Error {
            latency: DurationMs::from_millis(latency_ms),
            usage: Observed::declared_by_fixture(usage),
            error,
        }
    }
}

/// A deterministic adapter that validates configured provider/model identity
/// and returns a fixed observation without making a network call or sleeping.
#[derive(Debug, Clone)]
pub struct FakeProviderAdapter {
    provider: ProviderName,
    model: ProviderModel,
    response: FakeProviderResponse,
    secret: Option<ProviderSecret>,
}

impl FakeProviderAdapter {
    /// Configure the provider/model profile and fixed response.
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        response: FakeProviderResponse,
    ) -> Result<Self, ProviderContractError> {
        Ok(Self {
            provider: ProviderName::new(provider)?,
            model: ProviderModel::new(model)?,
            response,
            secret: None,
        })
    }

    /// Attach a credential solely to exercise safe adapter debug output.
    #[must_use]
    pub fn with_secret(mut self, secret: ProviderSecret) -> Self {
        self.secret = Some(secret);
        self
    }
}

impl ProviderOperationAdapter for FakeProviderAdapter {
    fn execute(
        &self,
        request: &ProviderOperationRequest,
    ) -> Result<ProviderOperationObservation, ProviderContractError> {
        let expected = ProviderIdentity::new(
            request.identity().operation_id().as_str(),
            self.provider.as_str(),
            self.model.as_str(),
        )?;
        if request.identity().provider() != expected.provider()
            || request.identity().model() != expected.model()
        {
            return Err(ProviderContractError::IdentityMismatch {
                expected,
                actual: request.identity().clone(),
            });
        }

        let observation = match &self.response {
            FakeProviderResponse::Success {
                latency,
                usage,
                finish,
            } => ProviderOperationObservation::new(
                request.clone(),
                Observed::declared_by_fixture(*latency),
                usage.clone(),
                Observed::declared_by_fixture(FinishClassification::Finished(*finish)),
            ),
            FakeProviderResponse::Error {
                latency,
                usage,
                error,
            } => ProviderOperationObservation::new(
                request.clone(),
                Observed::declared_by_fixture(*latency),
                usage.clone(),
                Observed::declared_by_fixture(FinishClassification::Error(*error)),
            ),
        };
        Ok(observation)
    }
}

/// Name emphasizing that the fake is deterministic, not externally authoritative.
pub type DeterministicProviderAdapter = FakeProviderAdapter;
