use std::time::Duration;

use made_core::error::DomainError;

use super::OpenAiApiKey;

const DEFAULT_ENDPOINT: &str = "https://api.openai.com";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_MAX_TOKENS: u32 = 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Static configuration for the OpenAI adapter. Every field is
/// validated on construction.
#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    pub(super) api_key: OpenAiApiKey,
    pub(super) endpoint: String,
    pub(super) model: String,
    pub(super) max_tokens: u32,
    pub(super) timeout: Duration,
}

impl OpenAiConfig {
    #[must_use]
    pub fn new(api_key: OpenAiApiKey) -> Self {
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
            super::super::endpoint::validate_provider_endpoint("openai.endpoint", endpoint)?;
        Ok(self)
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Result<Self, DomainError> {
        let value = model.into().trim().to_owned();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "openai.model",
            });
        }
        self.model = value;
        Ok(self)
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Result<Self, DomainError> {
        if max_tokens == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "openai.max_tokens",
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
