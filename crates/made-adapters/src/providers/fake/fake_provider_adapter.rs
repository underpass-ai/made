use super::super::operation::{
    FinishClassification, Observed, ProviderContractError, ProviderIdentity, ProviderModel,
    ProviderName, ProviderOperationAdapter, ProviderOperationObservation, ProviderOperationRequest,
    ProviderSecret,
};
use super::FakeProviderResponse;

/// A deterministic adapter that validates configured provider/model identity and returns a fixed observation.
#[derive(Debug, Clone)]
pub struct FakeProviderAdapter {
    provider: ProviderName,
    model: ProviderModel,
    response: FakeProviderResponse,
    secret: Option<ProviderSecret>,
}

impl FakeProviderAdapter {
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
