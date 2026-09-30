//! Backend failures of the SQLite authorization and budget stores.
//!
//! A SQLite failure is the server's problem, never the caller's. It
//! surfaces as `InvariantViolated` with a reason each store fixes, which
//! gRPC maps to `failed_precondition` and the embedded MCP to `refused`,
//! the same classification the Postgres authorization store kept in #266.
//! `DomainError` has no storage variant that carries runtime context, and
//! the one that does (`InvalidDocument`) would tell the client the failure
//! was its own (`invalid_argument` / `invalid_request`); see #268.
//!
//! The phase and the cause go to the structured log instead. The driver's
//! rendered text can repeat a stored value, a trigger's message or the
//! database path, so the cause is built from the error's *kind* only: the
//! SQLite result code and extended code, a column index, a value type.

use made_core::DomainError;

/// Map a rusqlite failure in `phase` to `InvariantViolated { reason }` and
/// log the phase and a sanitized cause.
pub(super) fn sqlite_backend_failure(
    error: &rusqlite::Error,
    phase: &'static str,
    reason: &'static str,
) -> DomainError {
    let detail = failure_detail(error, phase);
    tracing::error!(
        phase,
        reason,
        detail = %detail,
        "sqlite store operation failed"
    );
    DomainError::InvariantViolated { reason }
}

/// `<phase> failed: <cause>`, with no stored or configured values.
pub(super) fn failure_detail(error: &rusqlite::Error, phase: &'static str) -> String {
    format!("{phase} failed: {}", sanitized_cause(error))
}

fn sanitized_cause(error: &rusqlite::Error) -> String {
    if let Some(failure) = error.sqlite_error() {
        return format!(
            "sqlite error {:?} (extended code {})",
            failure.code, failure.extended_code
        );
    }
    match error {
        rusqlite::Error::SqliteSingleThreadedMode => "sqlite is in single-threaded mode".to_owned(),
        rusqlite::Error::FromSqlConversionFailure(index, kind, _) => {
            format!("column {index} could not be converted from {kind}")
        }
        rusqlite::Error::IntegralValueOutOfRange(index, _) => {
            format!("column {index} integer is out of range")
        }
        rusqlite::Error::Utf8Error(index, _) => format!("column {index} is not valid utf-8"),
        rusqlite::Error::NulError(_) => "text parameter contains a nul byte".to_owned(),
        rusqlite::Error::InvalidParameterName(_) => "invalid parameter name".to_owned(),
        rusqlite::Error::InvalidPath(_) => "invalid database path".to_owned(),
        rusqlite::Error::ExecuteReturnedResults => "execute returned rows".to_owned(),
        rusqlite::Error::QueryReturnedNoRows => "query returned no rows".to_owned(),
        rusqlite::Error::QueryReturnedMoreThanOneRow => {
            "query returned more than one row".to_owned()
        }
        rusqlite::Error::InvalidColumnIndex(index) => format!("column index {index} is invalid"),
        rusqlite::Error::InvalidColumnName(_) => "invalid column name".to_owned(),
        rusqlite::Error::InvalidColumnType(index, _, kind) => {
            format!("column {index} has unexpected type {kind}")
        }
        rusqlite::Error::StatementChangedRows(rows) => {
            format!("statement changed {rows} rows unexpectedly")
        }
        rusqlite::Error::ToSqlConversionFailure(_) => "parameter could not be converted".to_owned(),
        rusqlite::Error::InvalidQuery => "invalid query".to_owned(),
        rusqlite::Error::MultipleStatement => "multiple statements given".to_owned(),
        rusqlite::Error::InvalidParameterCount(given, expected) => {
            format!("{given} parameters given, {expected} expected")
        }
        _ => "driver error".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rusqlite::ffi;

    use super::*;

    const REASON: &str = "sqlite: test persistence backend failed";

    fn trigger_abort(message: &str) -> rusqlite::Error {
        rusqlite::Error::SqliteFailure(
            ffi::Error::new(ffi::SQLITE_CONSTRAINT_TRIGGER),
            Some(message.to_owned()),
        )
    }

    #[test]
    fn backend_failures_are_invariant_violations_not_invalid_documents() {
        assert_eq!(
            sqlite_backend_failure(&trigger_abort("x"), "append budget events", REASON),
            DomainError::InvariantViolated { reason: REASON }
        );
    }

    #[test]
    fn detail_names_the_phase_and_the_sqlite_codes() {
        assert_eq!(
            failure_detail(&trigger_abort("x"), "append budget events"),
            format!(
                "append budget events failed: sqlite error ConstraintViolation (extended code {})",
                ffi::SQLITE_CONSTRAINT_TRIGGER
            )
        );
    }

    #[test]
    fn driver_messages_paths_and_values_never_reach_the_detail() {
        let secret = "tenant-secret-42";
        let errors = [
            trigger_abort(&format!("row carries {secret}")),
            rusqlite::Error::InvalidPath(PathBuf::from(format!("/srv/{secret}/made.db"))),
            rusqlite::Error::IntegralValueOutOfRange(1, 4_242_424_242),
            rusqlite::Error::InvalidColumnName(secret.to_owned()),
            rusqlite::Error::ToSqlConversionFailure(secret.into()),
            rusqlite::Error::InvalidParameterName(secret.to_owned()),
        ];
        for error in errors {
            let detail = failure_detail(&error, "read authorization decision");
            assert!(!detail.contains(secret), "{detail}");
            assert!(!detail.contains("4242424242"), "{detail}");
            assert!(detail.starts_with("read authorization decision failed: "));
        }
    }

    #[test]
    fn non_driver_failures_keep_their_kind() {
        assert_eq!(
            failure_detail(&rusqlite::Error::QueryReturnedNoRows, "load budget events"),
            "load budget events failed: query returned no rows"
        );
        assert_eq!(
            failure_detail(
                &rusqlite::Error::InvalidColumnType(
                    2,
                    "payload".to_owned(),
                    rusqlite::types::Type::Text
                ),
                "load budget events"
            ),
            "load budget events failed: column 2 has unexpected type Text"
        );
    }
}
