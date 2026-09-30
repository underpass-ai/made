//! Errors of the Postgres authorization policy store that say what happened.
//!
//! The store keeps the domain error it has always returned for a backend
//! failure: `InvariantViolated`, which gRPC maps to `failed_precondition`.
//! `DomainError` has no storage variant that carries runtime context, and
//! the one that does (`InvalidDocument`) would tell the client the failure
//! was its own (`invalid_argument`); see #268.
//!
//! What changes is the log. The shared `ceremony_store::sqlx_error` logs
//! the driver's rendered text, which can echo a stored value (`invalid input
//! syntax for type ...: "<value>"`), a connection string or column contents.
//! Here the structured log carries the phase and a cause built from the
//! error's *kind* only: SQLSTATE, table, constraint, column, I/O kind.

use std::fmt::Write as _;

use made_core::DomainError;

/// The reason every backend failure of this store surfaces with. Distinct
/// from the ceremony store's, so a gate failure names the store at least.
pub(super) const AUTHORIZATION_BACKEND_FAILED: &str =
    "postgres: authorization persistence backend failed";

/// Map a sqlx failure in `phase` to the store's backend-failure error and
/// log the phase and a sanitized cause.
pub(super) fn authorization_sqlx_error(error: &sqlx::Error, phase: &'static str) -> DomainError {
    let detail = failure_detail(error, phase);
    tracing::error!(
        phase,
        detail = %detail,
        "postgres authorization store operation failed"
    );
    DomainError::InvariantViolated {
        reason: AUTHORIZATION_BACKEND_FAILED,
    }
}

/// `<phase> failed: <cause>`, with no stored or configured values.
pub(super) fn failure_detail(error: &sqlx::Error, phase: &'static str) -> String {
    format!("{phase} failed: {}", sanitized_cause(error))
}

fn sanitized_cause(error: &sqlx::Error) -> String {
    match error {
        sqlx::Error::Database(database) => {
            let mut cause = match database.code() {
                Some(code) => format!("database error SQLSTATE {code}"),
                None => "database error".to_owned(),
            };
            if let Some(table) = database.table() {
                let _ = write!(cause, " on table {table}");
            }
            if let Some(constraint) = database.constraint() {
                let _ = write!(cause, " (constraint {constraint})");
            }
            cause
        }
        sqlx::Error::Io(io) => format!("i/o error ({:?})", io.kind()),
        sqlx::Error::PoolTimedOut => "timed out acquiring a pooled connection".to_owned(),
        sqlx::Error::PoolClosed => "connection pool is closed".to_owned(),
        sqlx::Error::WorkerCrashed => "connection worker crashed".to_owned(),
        sqlx::Error::RowNotFound => "expected row not found".to_owned(),
        sqlx::Error::ColumnNotFound(column) => format!("column {column} not found"),
        sqlx::Error::ColumnIndexOutOfBounds { index, len } => {
            format!("column index {index} out of bounds ({len} columns)")
        }
        sqlx::Error::ColumnDecode { index, .. } => format!("column {index} could not be decoded"),
        sqlx::Error::TypeNotFound { type_name } => format!("type {type_name} not found"),
        sqlx::Error::Configuration(_) => "connection configuration rejected".to_owned(),
        sqlx::Error::Tls(_) => "tls failure".to_owned(),
        sqlx::Error::Protocol(_) => "wire protocol error".to_owned(),
        sqlx::Error::Encode(_) => "value could not be encoded".to_owned(),
        sqlx::Error::Decode(_) => "value could not be decoded".to_owned(),
        sqlx::Error::BeginFailed => "transaction could not begin".to_owned(),
        _ => "driver error".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_failures_keep_the_invariant_variant() {
        assert_eq!(
            authorization_sqlx_error(
                &sqlx::Error::PoolTimedOut,
                "lock authorization policy state"
            ),
            DomainError::InvariantViolated {
                reason: AUTHORIZATION_BACKEND_FAILED
            }
        );
    }

    #[test]
    fn detail_names_the_phase_and_the_cause() {
        assert_eq!(
            failure_detail(
                &sqlx::Error::PoolTimedOut,
                "lock authorization policy state"
            ),
            "lock authorization policy state failed: timed out acquiring a pooled connection"
        );
    }

    #[test]
    fn io_errors_carry_their_kind_but_not_their_message() {
        let error = sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "peer 10.0.0.7 said secret-token",
        ));
        let detail = failure_detail(&error, "begin authorization policy append");
        assert!(detail.ends_with("i/o error (ConnectionReset)"), "{detail}");
        assert!(!detail.contains("secret-token"));
        assert!(!detail.contains("10.0.0.7"));
    }

    #[test]
    fn configuration_and_driver_text_never_reaches_the_detail() {
        let secret = "postgres://made:hunter2@db.internal/made";
        let configuration = sqlx::Error::Configuration(secret.into());
        let protocol = sqlx::Error::Protocol(format!("unexpected message near {secret}"));
        let decode = sqlx::Error::Decode(format!("bad payload {secret}").into());
        for error in [configuration, protocol, decode] {
            let detail = failure_detail(&error, "read authorization decision");
            assert!(!detail.contains("hunter2"), "{detail}");
            assert!(!detail.contains("db.internal"), "{detail}");
        }
    }
}
