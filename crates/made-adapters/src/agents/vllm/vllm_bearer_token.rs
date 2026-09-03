use std::fmt;

use made_core::error::DomainError;

/// Opaque bearer token for vLLM deployments fronted by an auth proxy.
/// Its `Debug` impl is a fixed redaction. Construction rejects empty
/// values; if authentication is not needed, do not construct one.
#[derive(Clone)]
pub struct VllmBearerToken(String);

impl VllmBearerToken {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let trimmed = raw.into().trim().to_owned();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField {
                field: "vllm.bearer_token",
            });
        }
        Ok(Self(trimmed))
    }

    pub(super) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for VllmBearerToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VllmBearerToken(**redacted**)")
    }
}
