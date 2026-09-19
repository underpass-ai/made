use super::ProviderContractError;
use std::fmt;

/// Opaque credential that can be carried by an adapter without leaking in debug output.
#[derive(Clone)]
pub struct ProviderSecret(String);

impl ProviderSecret {
    pub fn new(raw: impl Into<String>) -> Result<Self, ProviderContractError> {
        let value = raw.into().trim().to_owned();
        if value.is_empty() {
            return Err(ProviderContractError::EmptyField { field: "secret" });
        }
        Ok(Self(value))
    }
}
impl fmt::Debug for ProviderSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _secret_is_present = !self.0.is_empty();
        f.write_str("ProviderSecret(**redacted**)")
    }
}
impl fmt::Display for ProviderSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("**redacted**")
    }
}
