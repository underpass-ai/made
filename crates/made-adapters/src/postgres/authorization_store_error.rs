//! Errors of the Postgres authorization policy store that say what happened.
//!
//! The shared `ceremony_store::sqlx_error` collapses every failure into
//! "postgres: ceremony persistence backend failed", which is also what the
//! authorization store used to surface: a gate failure then carried neither
//! the phase that failed nor why. This mirrors the SQLite store's
//! `sqlite_error` (#233, #246): the reason names the phase and the cause.
//!
//! The cause is built from the error's *kind*, never from its rendered text.
//! A Postgres message can echo the offending value (`invalid input syntax
//! for type ...: "<value>"`), a configuration error can carry the connection
//! string and a decode error the column contents, so only the SQLSTATE,
//! constraint, table, column and I/O kind reach the caller. The full error
//! still goes to the structured log.

use std::fmt::Write as _;

use made_core::DomainError;

/// Map a sqlx failure in `phase` to a domain error naming the phase and a
/// cause that carries no stored or configured values.
pub(super) fn authorization_sqlx_error(error: &sqlx::Error, phase: &'static str) -> DomainError {
    tracing::error!(%error, phase, "postgres authorization store operation failed");
    DomainError::InvalidDocument {
        reason: format!(
            "postgres authorization store: {phase} failed: {}",
            sanitized_cause(error)
        ),
    }
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

    fn reason(error: &sqlx::Error, phase: &'static str) -> String {
        match authorization_sqlx_error(error, phase) {
            DomainError::InvalidDocument { reason } => reason,
            other => panic!("unexpected variant {other:?}"),
        }
    }

    #[test]
    fn reason_names_the_phase_and_the_cause() {
        let reason = reason(
            &sqlx::Error::PoolTimedOut,
            "lock authorization policy state",
        );
        assert_eq!(
            reason,
            "postgres authorization store: lock authorization policy state failed: \
             timed out acquiring a pooled connection"
        );
        assert!(!reason.contains("ceremony persistence backend"));
    }

    #[test]
    fn io_errors_carry_their_kind_but_not_their_message() {
        let error = sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "peer 10.0.0.7 said secret-token",
        ));
        let reason = reason(&error, "begin authorization policy append");
        assert!(reason.ends_with("i/o error (ConnectionReset)"), "{reason}");
        assert!(!reason.contains("secret-token"));
        assert!(!reason.contains("10.0.0.7"));
    }

    #[test]
    fn configuration_and_driver_text_never_reaches_the_reason() {
        let secret = "postgres://made:hunter2@db.internal/made";
        let configuration = sqlx::Error::Configuration(secret.into());
        let protocol = sqlx::Error::Protocol(format!("unexpected message near {secret}"));
        let decode = sqlx::Error::Decode(format!("bad payload {secret}").into());
        for error in [configuration, protocol, decode] {
            let reason = reason(&error, "read authorization decision");
            assert!(!reason.contains("hunter2"), "{reason}");
            assert!(!reason.contains("db.internal"), "{reason}");
        }
    }
}
