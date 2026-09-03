use std::time::Duration;

use made_core::error::DomainError;

use super::{VllmBearerToken, VllmClientIdentity};

const DEFAULT_ENDPOINT: &str = "http://vllm-server:8000";
const DEFAULT_MAX_TOKENS: u32 = 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Static configuration for the vLLM adapter. Every field is
/// validated on construction.
///
/// Unlike `OpenAiConfig` this has no mandatory credential. Model
/// must be explicitly set — vLLM deployments serve whichever weights
/// the operator loaded; there is no sensible default.
#[derive(Debug, Clone)]
pub struct VllmConfig {
    pub(super) endpoint: String,
    pub(super) model: String,
    pub(super) bearer: Option<VllmBearerToken>,
    pub(super) client_identity: Option<VllmClientIdentity>,
    pub(super) max_tokens: u32,
    pub(super) timeout: Duration,
}

impl VllmConfig {
    /// Build a config. Model must be non-empty.
    pub fn new(model: impl Into<String>) -> Result<Self, DomainError> {
        let model = model.into().trim().to_owned();
        if model.is_empty() {
            return Err(DomainError::EmptyField {
                field: "vllm.model",
            });
        }
        Ok(Self {
            endpoint: DEFAULT_ENDPOINT.to_owned(),
            model,
            bearer: None,
            client_identity: None,
            max_tokens: DEFAULT_MAX_TOKENS,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Result<Self, DomainError> {
        self.endpoint =
            super::super::endpoint::validate_provider_endpoint("vllm.endpoint", endpoint)?;
        Ok(self)
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Result<Self, DomainError> {
        let value = model.into().trim().to_owned();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "vllm.model",
            });
        }
        self.model = value;
        Ok(self)
    }

    #[must_use]
    pub fn with_bearer(mut self, bearer: VllmBearerToken) -> Self {
        self.bearer = Some(bearer);
        self
    }

    /// Attach a client certificate + private key for mTLS-protected
    /// endpoints. The identity is handed to `reqwest` when the
    /// agent's HTTP client is built; if the PEM is malformed, the
    /// error surfaces at agent construction time.
    #[must_use]
    pub fn with_client_identity(mut self, identity: VllmClientIdentity) -> Self {
        self.client_identity = Some(identity);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Result<Self, DomainError> {
        if max_tokens == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "vllm.max_tokens",
            });
        }
        self.max_tokens = max_tokens;
        Ok(self)
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}
