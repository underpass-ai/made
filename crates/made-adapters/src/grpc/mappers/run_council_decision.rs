//! [`RunCouncilDecision`] proto ↔ use-case conversions.
//!
//! Input: validate the `selector` oneof (exactly one arm must be
//! set), enforce a non-empty `contract_id`, and parse the description
//! / external context / metadata. The mapper returns a small error
//! enum — [`RunCouncilDecisionInputError`] — so the service handler
//! can dispatch between `Status::invalid_argument` (transport-layer
//! validation: selector missing, contract_id blank) and
//! `Status::*` derived from the domain (`description` too long,
//! `metadata.source_event_id` malformed, …).
//!
//! Output: flatten the use-case output into the proto response. The
//! winner is materialised as a [`DeliberationResult`] proto carrying
//! the proposal, validation outcome, and `rank=0`. Candidates are
//! projected onto [`pb::CandidateSummary`] — a flatter shape geared
//! at consumers that want per-candidate signal without paying for
//! the full proposal payload.

use made_app::usecases::{RunCouncilDecisionInput, RunCouncilDecisionOutput};
use made_core::entities::RankedOutcome;
use made_core::error::DomainError;
use made_core::value_objects::{
    CouncilId, CouncilSelector, Specialty, TaskDescription, ValidationMode,
};
use made_proto::v1 as pb;
use tonic::Status;

use super::context::external_context_from_proto;
use super::proposal::proposal_to_proto;
use super::task::metadata_from_proto;
use super::validation::{validation_outcome_to_proto, validator_report_to_proto};

/// Reasons the proto request cannot be lifted into a use-case input.
#[derive(Debug)]
pub enum RunCouncilDecisionInputError {
    /// Transport-layer validation: a required scalar/oneof was missing
    /// or both selector arms were unexpectedly populated. Maps to
    /// `Code::InvalidArgument`.
    InvalidArgument(&'static str),
    /// A nested field failed domain validation (description too long,
    /// metadata event ids malformed, …). The handler routes this
    /// through `domain_error_to_status`.
    Domain(DomainError),
}

impl From<DomainError> for RunCouncilDecisionInputError {
    fn from(err: DomainError) -> Self {
        Self::Domain(err)
    }
}

impl From<RunCouncilDecisionInputError> for Status {
    fn from(err: RunCouncilDecisionInputError) -> Self {
        match err {
            RunCouncilDecisionInputError::InvalidArgument(reason) => {
                Status::invalid_argument(reason)
            }
            RunCouncilDecisionInputError::Domain(domain) => {
                super::super::status::domain_error_to_status(domain)
            }
        }
    }
}

/// Convert the proto request into the use-case input.
///
/// Selector validation rules:
/// * the `selector` oneof must be present (one arm set);
/// * both arms cannot be set — prost enforces this for `oneof`, so we
///   only reject the `None` (neither-set) case here;
/// * the chosen arm's string must satisfy domain validation
///   ([`Specialty::new`] / [`CouncilId::new`]).
pub fn run_council_decision_input_from_proto(
    proto: pb::RunCouncilDecisionRequest,
) -> Result<RunCouncilDecisionInput, RunCouncilDecisionInputError> {
    if proto.contract_id.trim().is_empty() {
        return Err(RunCouncilDecisionInputError::InvalidArgument(
            "contract_id is required",
        ));
    }

    let council_selector = match proto.selector {
        None => {
            return Err(RunCouncilDecisionInputError::InvalidArgument(
                "selector must be set (council_id or specialty)",
            ));
        }
        Some(pb::run_council_decision_request::Selector::CouncilId(id)) => {
            CouncilSelector::ById(CouncilId::new(id)?)
        }
        Some(pb::run_council_decision_request::Selector::Specialty(s)) => {
            CouncilSelector::BySpecialty(Specialty::new(s)?)
        }
    };

    let task_description = TaskDescription::new(proto.description)?;
    let external_context = external_context_from_proto(proto.external_context)?;
    let metadata = metadata_from_proto(proto.metadata)?;
    let validation_mode = validation_mode_from_proto(proto.validation_mode);

    Ok(RunCouncilDecisionInput {
        council_selector,
        contract_id: proto.contract_id,
        task_description,
        external_context,
        validation_mode,
        metadata,
    })
}

/// Materialise the use-case output into the proto response.
#[must_use]
pub fn run_council_decision_response_from(
    output: &RunCouncilDecisionOutput,
) -> pb::RunCouncilDecisionResponse {
    let candidates_total = u32::try_from(output.candidates.len()).unwrap_or(u32::MAX);
    let candidates_passed = u32::try_from(
        output
            .candidates
            .iter()
            .filter(|c| c.outcome().all_passed())
            .count(),
    )
    .unwrap_or(u32::MAX);

    let winner = Some(deliberation_result_from_ranked(&output.winner));
    let candidates = output
        .candidates
        .iter()
        .map(candidate_summary_from_ranked)
        .collect();

    pb::RunCouncilDecisionResponse {
        task_id: output.task_id.as_str().to_owned(),
        winner,
        validation: Some(pb::ValidationOutcomeSummary {
            passed: output.passed,
            candidates_passed,
            candidates_total,
        }),
        candidates,
        duration_ms: output.duration_ms.get(),
        validation_mode: validation_mode_to_proto(output.validation_mode) as i32,
    }
}

fn deliberation_result_from_ranked(outcome: &RankedOutcome) -> pb::DeliberationResult {
    pb::DeliberationResult {
        proposal: Some(proposal_to_proto(outcome.proposal())),
        validation: Some(validation_outcome_to_proto(outcome.outcome())),
        rank: outcome.rank(),
    }
}

fn candidate_summary_from_ranked(outcome: &RankedOutcome) -> pb::CandidateSummary {
    pb::CandidateSummary {
        proposal_id: outcome.proposal().id().as_str().to_owned(),
        author_agent_id: outcome.proposal().author().as_str().to_owned(),
        score: outcome.outcome().score().get(),
        reports: outcome
            .outcome()
            .reports()
            .iter()
            .map(validator_report_to_proto)
            .collect(),
        rank: outcome.rank(),
        passed: outcome.outcome().all_passed(),
        revision_count: outcome.proposal().revision_count(),
    }
}

fn validation_mode_from_proto(value: i32) -> ValidationMode {
    match pb::ValidationMode::try_from(value).unwrap_or(pb::ValidationMode::Unspecified) {
        // UNSPECIFIED → STRICT mirrors the proto comment on the field.
        pb::ValidationMode::Unspecified | pb::ValidationMode::Strict => ValidationMode::Strict,
        pb::ValidationMode::Warn => ValidationMode::Warn,
    }
}

const fn validation_mode_to_proto(mode: ValidationMode) -> pb::ValidationMode {
    match mode {
        ValidationMode::Strict => pb::ValidationMode::Strict,
        ValidationMode::Warn => pb::ValidationMode::Warn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::entities::{Proposal, RankedOutcome, ValidationOutcome, ValidatorReport};
    use made_core::value_objects::{
        AgentId, Attributes, DurationMs, ProposalId, Score, Specialty as VSpecialty, TaskId,
    };
    use time::macros::datetime;

    fn ranked(
        proposal_id: &str,
        agent_id: &str,
        score: f64,
        passed: bool,
        rank: u32,
    ) -> RankedOutcome {
        let proposal = Proposal::new(
            ProposalId::new(proposal_id).unwrap(),
            AgentId::new(agent_id).unwrap(),
            VSpecialty::new("triage").unwrap(),
            "content",
            Attributes::empty(),
            datetime!(2026-05-12 12:00:00 UTC),
        )
        .unwrap();
        let report =
            ValidatorReport::new("contract", passed, "summary", Attributes::empty()).unwrap();
        let outcome = ValidationOutcome::new(Score::new(score).unwrap(), vec![report]);
        // Use the public test constructor via serde to skip the private
        // RankedOutcome::new path (which doesn't exist as a pub fn).
        let json = serde_json::json!({
            "proposal": proposal,
            "outcome": outcome,
            "rank": rank,
        });
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn input_rejects_missing_selector() {
        let proto = pb::RunCouncilDecisionRequest {
            contract_id: "c1".into(),
            description: "decide".into(),
            ..Default::default()
        };
        let err = run_council_decision_input_from_proto(proto).unwrap_err();
        let status: Status = err.into();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("selector"));
    }

    #[test]
    fn input_rejects_empty_contract_id() {
        let proto = pb::RunCouncilDecisionRequest {
            contract_id: "   ".into(),
            description: "decide".into(),
            selector: Some(pb::run_council_decision_request::Selector::Specialty(
                "triage".into(),
            )),
            ..Default::default()
        };
        let err = run_council_decision_input_from_proto(proto).unwrap_err();
        let status: Status = err.into();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("contract_id"));
    }

    #[test]
    fn input_accepts_specialty_selector() {
        let proto = pb::RunCouncilDecisionRequest {
            contract_id: "c1".into(),
            description: "decide".into(),
            selector: Some(pb::run_council_decision_request::Selector::Specialty(
                "triage".into(),
            )),
            ..Default::default()
        };
        let input = run_council_decision_input_from_proto(proto).unwrap();
        match input.council_selector {
            CouncilSelector::BySpecialty(s) => assert_eq!(s.as_str(), "triage"),
            CouncilSelector::ById(_) => panic!("expected BySpecialty"),
        }
        assert_eq!(input.contract_id, "c1");
    }

    #[test]
    fn input_accepts_council_id_selector() {
        let proto = pb::RunCouncilDecisionRequest {
            contract_id: "c1".into(),
            description: "decide".into(),
            selector: Some(pb::run_council_decision_request::Selector::CouncilId(
                "council-x".into(),
            )),
            ..Default::default()
        };
        let input = run_council_decision_input_from_proto(proto).unwrap();
        match input.council_selector {
            CouncilSelector::ById(id) => assert_eq!(id.as_str(), "council-x"),
            CouncilSelector::BySpecialty(_) => panic!("expected ById"),
        }
    }

    #[test]
    fn response_carries_winner_candidates_and_validation_summary() {
        let winner = ranked("p1", "a1", 0.9, true, 0);
        let loser = ranked("p2", "a2", 0.3, false, 1);
        let output = RunCouncilDecisionOutput {
            task_id: TaskId::new("t1").unwrap(),
            winner: winner.clone(),
            candidates: vec![winner, loser],
            validation_mode: ValidationMode::Strict,
            passed: true,
            duration_ms: DurationMs::from_millis(123),
        };
        let resp = run_council_decision_response_from(&output);
        assert_eq!(resp.task_id, "t1");
        assert!(resp.winner.is_some());
        assert_eq!(resp.candidates.len(), 2);
        assert_eq!(resp.duration_ms, 123);
        let v = resp.validation.unwrap();
        assert!(v.passed);
        assert_eq!(v.candidates_total, 2);
        assert_eq!(v.candidates_passed, 1);
        assert_eq!(resp.validation_mode, pb::ValidationMode::Strict as i32);
    }

    #[test]
    fn warn_mode_with_failing_winner_round_trips() {
        let only = ranked("p1", "a1", 0.1, false, 0);
        let output = RunCouncilDecisionOutput {
            task_id: TaskId::new("t1").unwrap(),
            winner: only.clone(),
            candidates: vec![only],
            validation_mode: ValidationMode::Warn,
            passed: false,
            duration_ms: DurationMs::from_millis(10),
        };
        let resp = run_council_decision_response_from(&output);
        let v = resp.validation.unwrap();
        assert!(!v.passed);
        assert_eq!(v.candidates_passed, 0);
        assert_eq!(resp.validation_mode, pb::ValidationMode::Warn as i32);
    }
}
