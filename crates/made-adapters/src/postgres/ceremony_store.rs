use made_core::error::DomainError;
use serde::{Deserialize, Serialize};

use super::PostgresPool;

/// Shared Postgres implementation of the ceremony persistence ports.
#[derive(Debug, Clone)]
pub struct PostgresCeremonyStore {
    pub(super) pool: PostgresPool,
}

impl PostgresCeremonyStore {
    #[must_use]
    pub const fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }
}

pub(super) fn encode<T: Serialize>(
    value: &T,
    operation: &'static str,
) -> Result<Vec<u8>, DomainError> {
    serde_json::to_vec(value).map_err(|error| encoding_error(&error, operation))
}

pub(super) fn decode<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    operation: &'static str,
) -> Result<T, DomainError> {
    serde_json::from_slice(bytes).map_err(|error| decoding_error(&error, operation))
}

pub(super) fn sqlx_error(error: sqlx::Error, operation: &'static str) -> DomainError {
    tracing::error!(%error, operation, "postgres ceremony store operation failed");
    drop(error);
    DomainError::InvariantViolated {
        reason: "postgres: ceremony persistence backend failed",
    }
}

fn encoding_error(error: &serde_json::Error, operation: &'static str) -> DomainError {
    tracing::error!(%error, operation, "postgres ceremony store payload failed validation");
    DomainError::InvariantViolated {
        reason: "postgres: ceremony payload could not be serialized",
    }
}

fn decoding_error(error: &serde_json::Error, operation: &'static str) -> DomainError {
    tracing::error!(%error, operation, "postgres ceremony store payload failed validation");
    DomainError::InvariantViolated {
        reason: "postgres: ceremony payload could not be deserialized",
    }
}

pub(super) fn u64_to_i64(value: u64) -> Result<i64, DomainError> {
    i64::try_from(value).map_err(|_| DomainError::InvariantViolated {
        reason: "postgres: ceremony coordinate exceeds bigint",
    })
}

pub(super) fn i64_to_u64(value: i64) -> Result<u64, DomainError> {
    u64::try_from(value).map_err(|_| DomainError::InvariantViolated {
        reason: "postgres: ceremony coordinate is negative",
    })
}
