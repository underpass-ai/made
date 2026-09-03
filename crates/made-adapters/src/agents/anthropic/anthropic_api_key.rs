use std::fmt;

use made_core::error::DomainError;

/// Opaque API key. Its `Debug` impl is a fixed redaction so the
/// secret value cannot slip into logs, event payloads, or test
/// snapshots by accident.
#[derive(Clone)]
pub struct AnthropicApiKey(String);

impl AnthropicApiKey {
    /// Validate and construct. Empty / whitespace-only keys are
    /// rejected at the boundary so a misconfigured deployment fails
    /// fast instead of receiving a 401 on the first request.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let trimmed = raw.into().trim().to_owned();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField {
                field: "anthropic.api_key",
            });
        }
        Ok(Self(trimmed))
    }

    pub(super) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AnthropicApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AnthropicApiKey(**redacted**)")
    }
}
