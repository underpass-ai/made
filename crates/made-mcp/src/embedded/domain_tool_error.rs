//! `DomainError` → [`ToolError`], in one place.
//!
//! The in-process backend used to answer whatever `Display` said, so a
//! client had to read prose to tell "that session does not exist" from
//! "the engine will not do that". The split below is the same one the
//! server already makes in `domain_error_to_status`, which is why the
//! two backends now agree on a code without either arm knowing about
//! the other.

use made_core::error::DomainError;
use made_core::ports::ArtifactStoreError;
use made_core::BudgetError;

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
            // A write that lost a race. Worth reading the session again
            // and repeating, which is what the server's own mapper says
            // by answering `aborted` and what `AppendOutcome`'s doc says
            // about the outcome that produces it.
            DomainError::Conflict { .. } => Self::conflict(message),
            // The engine having looked: an illegal transition, a
            // violated invariant, a session that already exists, a
            // proposal nothing validated, a record no reader can read.
            //
            // Written out rather than left to a `_`, because the server
            // mapper in `made-adapters` is exhaustive and the two have
            // to agree: a variant added to `DomainError` must make
            // somebody choose a code on both arms, not silently become a
            // refusal on one. `mcp_error_vocabulary.rs` composes the two
            // and compares them on every variant there is.
            DomainError::InvalidTransition { .. }
            | DomainError::InvariantViolated { .. }
            | DomainError::AlreadyExists { .. }
            | DomainError::NoValidProposal { .. }
            | DomainError::LifecycleRefused { .. }
            | DomainError::UnreadableCeremonyEvent { .. } => Self::refused(message),
        }
    }
}

impl From<ArtifactStoreError> for ToolError {
    fn from(error: ArtifactStoreError) -> Self {
        let message = error.to_string();
        match error {
            ArtifactStoreError::Invalid(error) => error.into(),
            ArtifactStoreError::NotFound => Self::not_found(message),
            ArtifactStoreError::UnexpectedOffset { .. }
            | ArtifactStoreError::UploadCommitted
            | ArtifactStoreError::UploadAborted
            | ArtifactStoreError::IdempotencyConflict => Self::conflict(message),
            ArtifactStoreError::StorageUnavailable => Self::unavailable(message),
            ArtifactStoreError::ArtifactTooLarge { .. }
            | ArtifactStoreError::ChunkTooLarge { .. }
            | ArtifactStoreError::ChunkDigestMismatch
            | ArtifactStoreError::Incomplete { .. }
            | ArtifactStoreError::FinalDigestMismatch
            | ArtifactStoreError::Tombstoned
            | ArtifactStoreError::AccessDenied
            | ArtifactStoreError::InvalidBackup
            | ArtifactStoreError::InvalidCursor => Self::refused(message),
        }
    }
}

impl From<BudgetError> for ToolError {
    fn from(error: BudgetError) -> Self {
        let message = error.to_string();
        match error {
            BudgetError::Persistence(error) => error.into(),
            BudgetError::LedgerNotOpen | BudgetError::ReservationNotFound(_) => {
                Self::not_found(message)
            }
            BudgetError::ReservationConflict(_) | BudgetError::ReconciliationConflict(_) => {
                Self::conflict(message)
            }
            BudgetError::Exhausted { .. } | BudgetError::MissingReservationEstimate(_) => {
                Self::refused(message)
            }
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

    /// A lost race is not a refusal, and a client that cannot tell
    /// them apart retries the wrong ones.
    #[test]
    fn a_lost_race_is_worth_repeating() {
        let error = ToolError::from(DomainError::Conflict {
            what: "ceremony_instance",
        });
        assert_eq!(error.code(), ToolErrorCode::Conflict);
        assert!(error.is_retryable());
    }

    #[test]
    fn the_engine_having_looked_is_a_refusal() {
        for error in [
            DomainError::InvalidTransition {
                from: "OPEN",
                to: "DONE",
            },
            DomainError::InvariantViolated { reason: "no" },
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
