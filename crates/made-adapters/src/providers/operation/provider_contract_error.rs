use super::ProviderIdentity;
use thiserror::Error;

/// Contract errors are safe to expose in logs and test snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderContractError {
    #[error("provider contract field `{field}` must not be empty")]
    EmptyField { field: &'static str },
    #[error("provider adapter identity mismatch: expected {expected:?}, got {actual:?}")]
    IdentityMismatch {
        expected: ProviderIdentity,
        actual: ProviderIdentity,
    },
}
