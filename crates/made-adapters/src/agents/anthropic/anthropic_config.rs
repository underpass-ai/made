use std::time::Duration;

use made_core::error::DomainError;

use super::AnthropicApiKey;

const DEFAULT_ENDPOINT: &str = "https://api.anthropic.com";
const DEFAULT_MODEL: &str = "claude-haiku-4-5-20251001";
const DEFAULT_MAX_TOKENS: u32 = 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Static configuration for the Anthropic adapter.
///
/// All fields are validated on construction. Defaults match the
/// Messages API's current conventions.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    pub(super) api_key: AnthropicApiKey,
    pub(super) endpoint: String,
    pub(super) model: String,
    pub(super) max_tokens: u32,
    pub(super) timeout: Duration,
}

impl AnthropicConfig {
    #[must_use]
    pub fn new(api_key: AnthropicApiKey) -> Self {
        Self {
            api_key,
            endpoint: DEFAULT_ENDPOINT.to_owned(),
            model: DEFAULT_MODEL.to_owned(),
            max_tokens: DEFAULT_MAX_TOKENS,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Result<Self, DomainError> {
        self.endpoint =
            super::super::endpoint::validate_provider_endpoint("anthropic.endpoint", endpoint)?;
        Ok(self)
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Result<Self, DomainError> {
        let value = model.into().trim().to_owned();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "anthropic.model",
            });
        }
        self.model = value;
        Ok(self)
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Result<Self, DomainError> {
        if max_tokens == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "anthropic.max_tokens",
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
