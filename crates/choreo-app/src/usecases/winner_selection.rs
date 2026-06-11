//! Winner-selection policy for a completed deliberation.
//!
//! Given the ranked outcomes and the task constraints, decide which
//! proposal wins. This is policy, distinct from the deliberation pipeline
//! that produced the ranking: under an output contract the winner must be
//! *valid* (it passed every validator), and valid proposals are surfaced
//! ahead of invalid ones; without a contract the top-ranked proposal wins.

use choreo_core::entities::{Deliberation, RankedOutcome, TaskConstraints};
use choreo_core::error::DomainError;

/// Reorder the deliberation's sealed ranking so contract-valid proposals
/// (those that passed every validator) come before invalid ones, and
/// return the reordered outcomes. Used only when an output contract is in
/// force, so a valid proposal is never ranked behind an invalid one.
pub(super) fn prioritize_valid_outputs(
    deliberation: &mut Deliberation,
    ranked: &[RankedOutcome],
) -> Result<Vec<RankedOutcome>, DomainError> {
    let reordered = ranked
        .iter()
        .filter(|candidate| candidate.outcome().all_passed())
        .chain(
            ranked
                .iter()
                .filter(|candidate| !candidate.outcome().all_passed()),
        )
        .map(|candidate| candidate.proposal().id().clone())
        .collect::<Vec<_>>();
    deliberation.reprioritize(reordered)
}

/// Pick the winning outcome. Under an output contract the winner is the
/// first proposal that passed every validator (a [`DomainError::NoValidProposal`]
/// when none did); otherwise it is the top-ranked proposal.
pub(super) fn pick_winner<'a>(
    ranked: &'a [RankedOutcome],
    constraints: &TaskConstraints,
) -> Result<&'a RankedOutcome, DomainError> {
    if let Some(contract) = constraints.output_contract() {
        return ranked
            .iter()
            .find(|candidate| candidate.outcome().all_passed())
            .ok_or(DomainError::NoValidProposal {
                contract_id: contract.contract_id().to_owned(),
            });
    }

    ranked.first().ok_or(DomainError::EmptyCollection {
        field: "deliberation.ranked",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use choreo_core::entities::{Proposal, ValidationOutcome, ValidatorReport};
    use choreo_core::value_objects::{
        AgentId, Attributes, OutputContract, OutputFieldRule, ProposalId, Rounds, Rubric, Score,
        Specialty,
    };
    use std::collections::BTreeMap;
    use time::macros::datetime;

    fn outcome(id: &str, score: f64, all_passed: bool) -> RankedOutcome {
        let report =
            ValidatorReport::new("structural", all_passed, "", Attributes::empty()).unwrap();
        let proposal = Proposal::new(
            ProposalId::new(id).unwrap(),
            AgentId::new("agent").unwrap(),
            Specialty::new("reviewer").unwrap(),
            "content".to_owned(),
            Attributes::empty(),
            datetime!(2026-04-15 12:00:00 UTC),
        )
        .unwrap();
        RankedOutcome::new(
            proposal,
            ValidationOutcome::new(Score::new(score).unwrap(), vec![report]),
            0,
        )
    }

    fn contract_constraints() -> TaskConstraints {
        TaskConstraints::new(Rubric::empty(), Rounds::new(0).unwrap(), None, None)
            .with_output_contract(
                OutputContract::json_object(
                    "c",
                    BTreeMap::from([(
                        "decision".to_owned(),
                        OutputFieldRule::new(true, ["emit_event"]).unwrap(),
                    )]),
                )
                .unwrap(),
            )
    }

    #[test]
    fn picks_top_ranked_without_a_contract() {
        let ranked = [outcome("a", 0.9, true), outcome("b", 0.5, true)];
        let winner = pick_winner(&ranked, &TaskConstraints::default()).unwrap();
        assert_eq!(winner.proposal().id().as_str(), "a");
    }

    #[test]
    fn picks_first_valid_under_a_contract() {
        // "a" is top-ranked but invalid; "b" is the first valid proposal.
        let ranked = [outcome("a", 0.9, false), outcome("b", 0.5, true)];
        let winner = pick_winner(&ranked, &contract_constraints()).unwrap();
        assert_eq!(winner.proposal().id().as_str(), "b");
    }

    #[test]
    fn no_valid_proposal_under_a_contract_when_none_pass() {
        let ranked = [outcome("a", 0.9, false), outcome("b", 0.5, false)];
        let err = pick_winner(&ranked, &contract_constraints()).unwrap_err();
        assert!(matches!(err, DomainError::NoValidProposal { .. }));
    }

    #[test]
    fn empty_ranking_without_a_contract_is_rejected() {
        let err = pick_winner(&[], &TaskConstraints::default()).unwrap_err();
        assert!(matches!(
            err,
            DomainError::EmptyCollection {
                field: "deliberation.ranked"
            }
        ));
    }
}
