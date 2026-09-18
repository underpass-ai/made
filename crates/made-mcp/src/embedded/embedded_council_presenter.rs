use made_app::usecases::{DeliberateOutput, OrchestrateOutput, RunCouncilDecisionOutput};
use made_core::entities::{Council, Deliberation, Proposal, RankedOutcome, ValidationOutcome};
use made_core::value_objects::{OutputContract, ValidationMode};
use serde_json::{json, Map, Value};
use time::format_description::well_known::Rfc3339;

pub(super) fn deliberate(output: &DeliberateOutput) -> Value {
    deliberate_from(&output.deliberation, output.winner_proposal_id.as_str())
}

pub(super) fn deliberation(deliberation: &Deliberation) -> Value {
    let winner = deliberation
        .ranking()
        .first()
        .map_or("", made_core::value_objects::ProposalId::as_str);
    deliberate_from(deliberation, winner)
}

fn deliberate_from(deliberation: &Deliberation, winner: &str) -> Value {
    json!({
        "task_id": deliberation.task_id().as_str(),
        "winner_proposal_id": winner,
        "duration_ms": deliberation.duration().map_or(0, made_core::value_objects::DurationMs::get),
        "results": ranked(deliberation),
        "metadata": {},
    })
}

pub(super) fn orchestrate(output: &OrchestrateOutput) -> Value {
    let mut candidates = ranked(&output.deliberation);
    let winner_id = output.winner.id().as_str();
    let winner = candidates
        .iter()
        .find(|value| value["proposal"]["proposal_id"] == winner_id)
        .cloned()
        .unwrap_or(Value::Null);
    candidates.retain(|value| value["proposal"]["proposal_id"] != winner_id);
    json!({
        "task_id": output.deliberation.task_id().as_str(),
        "execution_id": output.execution.id().as_str(),
        "duration_ms": output.execution.duration().get(),
        "winner": winner,
        "candidates": candidates,
        "metadata": {},
    })
}

pub(super) fn council(council: &Council) -> Value {
    json!({
        "specialty": council.specialty().as_str(),
        "num_agents": u32::try_from(council.size()).unwrap_or(u32::MAX),
        "created_at": council.created_at().format(&Rfc3339).unwrap_or_default(),
        "agents": [],
    })
}

pub(super) fn contract(contract: &OutputContract) -> Value {
    let fields: Map<String, Value> = contract
        .fields()
        .iter()
        .map(|(name, rule)| {
            (
                name.clone(),
                json!({
                    "required": rule.required(),
                    "allowed_string_values": rule.allowed_string_values(),
                }),
            )
        })
        .collect();
    json!({
        "contract_id": contract.contract_id().as_str(),
        "format": contract.format().as_str(),
        "fields": fields,
        "json_schema": contract.json_schema(),
    })
}

pub(super) fn run_council_decision(output: &RunCouncilDecisionOutput) -> Value {
    let candidates_passed = output
        .candidates
        .iter()
        .filter(|candidate| candidate.outcome().all_passed())
        .count();
    json!({
        "task_id": output.task_id.as_str(),
        "winner": ranked_outcome(&output.winner),
        "validation": {
            "passed": output.passed.get(),
            "candidates_passed": u32::try_from(candidates_passed).unwrap_or(u32::MAX),
            "candidates_total": u32::try_from(output.candidates.len()).unwrap_or(u32::MAX),
        },
        "candidates": output.candidates.iter().map(candidate).collect::<Vec<_>>(),
        "duration_ms": output.duration_ms.get(),
        "validation_mode": match output.validation_mode {
            ValidationMode::Strict => "VALIDATION_MODE_STRICT",
            ValidationMode::Warn => "VALIDATION_MODE_WARN",
        },
    })
}

pub(super) fn result_frame(output: &DeliberateOutput) -> Value {
    let result = ranked(&output.deliberation)
        .into_iter()
        .next()
        .unwrap_or(Value::Null);
    json!({
        "task_id": output.deliberation.task_id().as_str(),
        "phase": "DELIBERATION_PHASE_COMPLETED",
        "emitted_at": Value::Null,
        "payload": { "kind": "result", "result": result },
    })
}

fn ranked(deliberation: &Deliberation) -> Vec<Value> {
    deliberation
        .ranking()
        .iter()
        .enumerate()
        .filter_map(|(rank, id)| {
            Some(result(
                u32::try_from(rank).unwrap_or(u32::MAX),
                deliberation.proposals().get(id)?,
                deliberation.outcomes().get(id)?,
            ))
        })
        .collect()
}

fn ranked_outcome(outcome: &RankedOutcome) -> Value {
    result(outcome.rank(), outcome.proposal(), outcome.outcome())
}

fn result(rank: u32, proposal: &Proposal, outcome: &ValidationOutcome) -> Value {
    json!({
        "rank": rank,
        "proposal": proposal_value(proposal),
        "validation": validation(outcome),
    })
}

fn proposal_value(proposal: &Proposal) -> Value {
    json!({
        "proposal_id": proposal.id().as_str(),
        "author_agent_id": proposal.author().as_str(),
        "content": proposal.content().as_str(),
        "metadata": proposal.attributes().as_map(),
        "revision_count": proposal.revision_count(),
    })
}

fn validation(outcome: &ValidationOutcome) -> Value {
    json!({
        "score": outcome.score().get(),
        "passed": outcome.all_passed(),
        "reports": outcome.reports().iter().map(|report| json!({
            "kind": report.kind(),
            "passed": report.passed(),
            "summary": report.summary(),
            "details": report.details().as_map(),
        })).collect::<Vec<_>>(),
    })
}

fn candidate(outcome: &RankedOutcome) -> Value {
    json!({
        "proposal_id": outcome.proposal().id().as_str(),
        "author_agent_id": outcome.proposal().author().as_str(),
        "score": outcome.outcome().score().get(),
        "reports": outcome.outcome().reports().iter().map(|report| json!({
            "kind": report.kind(),
            "passed": report.passed(),
            "summary": report.summary(),
            "details": report.details().as_map(),
        })).collect::<Vec<_>>(),
        "rank": outcome.rank(),
        "passed": outcome.outcome().all_passed(),
        "revision_count": outcome.proposal().revision_count(),
    })
}
