//! Map redb and serde failures to [`DomainError`].
//!
//! The core error enum carries static strings only — runtime detail
//! belongs in structured logs, not in a variant payload. Centralised
//! here so every redb adapter logs the original identically and
//! surfaces a small, stable set of domain variants upward.

use made_core::error::DomainError;

pub(super) fn store_failure(error: impl std::fmt::Display, op: &'static str) -> DomainError {
    let rendered = error.to_string();
    tracing::error!(error = %rendered, operation = op, "redb operation failed");
    // A lock held by another process is the one store failure an operator
    // hits routinely and can act on — two agent hosts opening the same file
    // — so it earns its own stable reason instead of being flattened into
    // the generic one, which left the caller unable to say anything useful.
    // Still a static string: no runtime detail crosses into the payload.
    if rendered.contains("Cannot acquire lock") {
        return DomainError::InvariantViolated {
            reason: "redb: the store is already open by another process",
        };
    }
    DomainError::InvariantViolated {
        reason: "redb: persistence backend failed",
    }
}

pub(super) fn encoding_failure(error: &serde_json::Error, op: &'static str) -> DomainError {
    tracing::error!(error = %error, operation = op, "redb record serde failed");
    DomainError::InvariantViolated {
        reason: "redb: stored record could not be encoded or decoded",
    }
}

/// A blocking store operation that never returned.
pub(super) fn join_failure(error: &tokio::task::JoinError, op: &'static str) -> DomainError {
    tracing::error!(error = %error, operation = op, "redb blocking task failed");
    DomainError::InvariantViolated {
        reason: "redb: storage task did not complete",
    }
}
