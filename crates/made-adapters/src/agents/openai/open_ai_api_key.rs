use std::fmt;

use made_core::error::DomainError;

/// Opaque API key. Its `Debug` impl is a fixed redaction so the
/// secret value cannot slip into logs, event payloads, or test
/// snapshots.
#[derive(Clone)]
pub struct OpenAiApiKey(String);

impl OpenAiApiKey {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let trimmed = raw.into().trim().to_owned();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField {
                field: "openai.api_key",
            });
        }
        Ok(Self(trimmed))
    }

    pub(super) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for OpenAiApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OpenAiApiKey(**redacted**)")
    }
}
