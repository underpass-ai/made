//! `DomainError` → [`ToolError`], in one place.
//!
//! The in-process backend used to answer whatever `Display` said, so a
//! client had to read prose to tell "that session does not exist" from
//! "the engine will not do that". The split below is the same one the
//! server already makes in `domain_error_to_status`, which is why the
//! two backends now agree on a code without either arm knowing about
//! the other.

use made_core::error::DomainError;

use crate::protocol::ToolError;

impl From<DomainError> for ToolError {
    fn from(error: DomainError) -> Self {
        let message = error.to_string();
        match error {
            // The value objects' own complaints: a field that is empty,
            // too long, mistyped or outside its range. The caller can
            // fix every one of them without knowing the session.
            DomainError::EmptyField { .. }
            | DomainError::FieldTooLong { .. }
            | DomainError::InvalidCharacters { .. }
            | DomainError::OutOfRange { .. }
            | DomainError::MustBeNonZero { .. }
            | DomainError::EmptyCollection { .. }
            // A document the caller wrote whose parts do not fit
            // together: the reason names the element at fault, and
            // fixing it needs nothing the caller does not have.
            | DomainError::InvalidDocument { .. } => Self::invalid_request(message),
            DomainError::NotFound { .. } => Self::not_found(message),
            // Everything else is the engine having looked: an illegal
            // transition, a violated invariant, a lost race, a session
            // that already exists, a record no reader can read.
            _ => Self::refused(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ToolErrorCode;

    #[test]
    fn a_missing_field_is_the_callers_to_fix() {
        assert_eq!(
            ToolError::from(DomainError::EmptyField {
                field: "ceremony_id"
            })
            .code(),
            ToolErrorCode::InvalidRequest
        );
    }

    /// The same classification the server gives a document it cannot
    /// build: the caller wrote it, the caller can fix it.
    #[test]
    fn a_document_whose_parts_do_not_fit_is_the_callers_to_fix() {
        let error = ToolError::from(DomainError::InvalidDocument {
            reason: "stage `review` names unknown owner role `MISSING`".to_owned(),
        });
        assert_eq!(error.code(), ToolErrorCode::InvalidRequest);
        assert_eq!(
            error.message(),
            "stage `review` names unknown owner role `MISSING`"
        );
    }

    #[test]
    fn a_lookup_that_did_not_resolve_is_not_found() {
        assert_eq!(
            ToolError::from(DomainError::NotFound {
                what: "ceremony_instance"
            })
            .code(),
            ToolErrorCode::NotFound
        );
    }

    #[test]
    fn the_engine_having_looked_is_a_refusal() {
        for error in [
            DomainError::InvalidTransition {
                from: "OPEN",
                to: "DONE",
            },
            DomainError::InvariantViolated { reason: "no" },
            DomainError::Conflict {
                what: "ceremony_instance",
            },
            DomainError::AlreadyExists {
                what: "ceremony_instance",
            },
        ] {
            assert_eq!(
                ToolError::from(error).code(),
                ToolErrorCode::Refused,
                "the engine looked and said no"
            );
        }
    }
}
