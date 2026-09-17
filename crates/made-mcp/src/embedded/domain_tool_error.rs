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
            // mapper next door is exhaustive and the two have to agree:
            // a variant added to `DomainError` must make somebody choose
            // a code on both arms, not silently become a refusal on one.
            DomainError::InvalidTransition { .. }
            | DomainError::InvariantViolated { .. }
            | DomainError::AlreadyExists { .. }
            | DomainError::NoValidProposal { .. }
            | DomainError::UnreadableCeremonyEvent { .. } => Self::refused(message),
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

/// The two mappers, compared on every variant there is.
///
/// One failure raised in this process and the same failure crossing the
/// wire must reach a client as the same word. Until now that was a
/// claim in a doc comment and a `_` arm on this side: the server mapper
/// was exhaustive, this one was not, so a variant added to
/// `DomainError` would have become `failed_precondition` over there and
/// `refused` here without anybody choosing either.
///
/// So the claim is a test, and it is the composition that is tested —
/// `DomainError` → `Status` → `ToolError` against `DomainError` →
/// `ToolError` — rather than two lists that have to be kept the same.
/// No server, no store, microseconds; it needs both backends compiled
/// in, which is what `cargo test -p made-mcp` uses.
#[cfg(all(test, feature = "grpc"))]
mod both_arms {
    use made_adapters::grpc::domain_error_to_status;
    use made_core::error::DomainError;

    use crate::protocol::ToolError;

    /// Every variant of `DomainError`, one example each. A variant
    /// added to core makes this list fail to compile against the match
    /// below before it can fail at runtime.
    fn every_domain_error() -> Vec<DomainError> {
        let all = vec![
            DomainError::EmptyField {
                field: "ceremony_id",
            },
            DomainError::FieldTooLong {
                field: "why",
                actual: 10,
                max: 5,
            },
            DomainError::InvalidCharacters { field: "step_id" },
            DomainError::OutOfRange {
                field: "score",
                value: 1.5,
                min: 0.0,
                max: 1.0,
            },
            DomainError::MustBeNonZero {
                field: "lease_ttl_ms",
            },
            DomainError::EmptyCollection {
                field: "ceremony_ids",
            },
            DomainError::InvalidTransition {
                from: "OPEN",
                to: "DONE",
            },
            DomainError::InvariantViolated {
                reason: "a step resolved twice",
            },
            DomainError::NotFound {
                what: "ceremony_instance",
            },
            DomainError::InvalidDocument {
                reason: "stage `review` names unknown owner role `MISSING`".to_owned(),
            },
            DomainError::AlreadyExists {
                what: "ceremony_instance",
            },
            DomainError::Conflict {
                what: "ceremony_instance",
            },
            DomainError::NoValidProposal {
                contract_id: "decision-contract".to_owned(),
            },
            DomainError::UnreadableCeremonyEvent {
                event_type: "step_completed",
                version: 2,
                reason: "no reader exists for this schema version",
            },
        ];
        // The compiler's own list, so a variant added to core lands
        // here rather than being quietly left out of the comparison.
        for error in &all {
            match error {
                DomainError::EmptyField { .. }
                | DomainError::FieldTooLong { .. }
                | DomainError::InvalidCharacters { .. }
                | DomainError::OutOfRange { .. }
                | DomainError::MustBeNonZero { .. }
                | DomainError::EmptyCollection { .. }
                | DomainError::InvalidTransition { .. }
                | DomainError::InvariantViolated { .. }
                | DomainError::NotFound { .. }
                | DomainError::InvalidDocument { .. }
                | DomainError::AlreadyExists { .. }
                | DomainError::Conflict { .. }
                | DomainError::NoValidProposal { .. }
                | DomainError::UnreadableCeremonyEvent { .. } => {}
            }
        }
        all
    }

    #[test]
    fn one_failure_reaches_a_client_as_one_word_whichever_arm_carried_it() {
        for error in every_domain_error() {
            let over_the_wire = ToolError::from(domain_error_to_status(error.clone())).code();
            let in_process = ToolError::from(error.clone()).code();
            assert_eq!(
                over_the_wire, in_process,
                "`{error:?}` is `{over_the_wire}` over the wire and `{in_process}` in process; \
                 a client would have to know which engine answered in order to read the failure"
            );
        }
    }

    /// And retryability follows the word, so the two arms cannot
    /// disagree about whether the call is worth repeating either.
    #[test]
    fn a_lost_race_is_retryable_on_both_arms() {
        let error = DomainError::Conflict {
            what: "ceremony_instance",
        };
        assert!(ToolError::from(domain_error_to_status(error.clone())).is_retryable());
        assert!(ToolError::from(error).is_retryable());
    }
}
