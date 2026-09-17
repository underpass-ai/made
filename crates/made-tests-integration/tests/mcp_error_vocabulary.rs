//! The two mappers of the one error vocabulary, compared on every
//! variant there is.
//!
//! One failure raised in this process and the same failure crossing the
//! wire must reach a client as the same word. Until now that was a
//! claim in a doc comment and a `_` arm on this side: the server mapper
//! was exhaustive, this one was not, so a variant added to
//! `DomainError` would have become `failed_precondition` over there and
//! `refused` here without anybody choosing either.
//!
//! So the claim is a test, and it is the composition that is tested —
//! `DomainError` → `Status` → `ToolError` against `DomainError` →
//! `ToolError` — rather than two lists that have to be kept the same.
//! No server, no store, microseconds. It lives here, and not beside
//! `From<DomainError> for ToolError` in `made-mcp`, for a build reason
//! rather than a design one: reaching `domain_error_to_status` from
//! there means a dev-dependency on `made-adapters`' `grpc` feature,
//! which puts `made-proto` — and so `protoc` — into the test targets of
//! a `made-mcp` build that has no backend at all, and the embedded
//! dependency-boundary job lints exactly that build without `protoc`
//! installed. This crate already depends on both, for exactly this kind
//! of comparison.
use made_adapters::grpc::domain_error_to_status;
use made_core::error::DomainError;

use made_mcp::protocol::ToolError;

/// Every variant of `DomainError`, one example each. A variant added to
/// core makes this list fail to compile against the match below before
/// it can fail at runtime.
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

/// And retryability follows the word, so the two arms cannot disagree
/// about whether the call is worth repeating either.
#[test]
fn a_lost_race_is_retryable_on_both_arms() {
    let error = DomainError::Conflict {
        what: "ceremony_instance",
    };
    assert!(ToolError::from(domain_error_to_status(error.clone())).is_retryable());
    assert!(ToolError::from(error).is_retryable());
}
