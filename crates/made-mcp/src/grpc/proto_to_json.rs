//! proto-response → JSON mappers.
//!
//! Mirror of `json_to_proto.rs`. Every proto field gets a named JSON
//! key explicitly so the wire schema is documented in the code and
//! reviewers can spot accidental drops at PR time.

use made_mcp_proto::v1 as pb;
use serde_json::{json, Map, Number as JsonNumber, Value};

mod ceremony_instance;
mod primitives;

pub(crate) use ceremony_instance::{
    ceremony_instance_listing_to_json, ceremony_instance_state_to_json,
};
use primitives::phase_name;
pub(crate) use primitives::{optional_pb_struct_to_json, pb_struct_to_json, timestamp_to_rfc3339};

// ---------------------------------------------------------------------------
// Composite responses
// ---------------------------------------------------------------------------

pub(crate) fn proposal_to_json(p: pb::Proposal) -> Value {
    json!({
        "proposal_id": p.proposal_id,
        "author_agent_id": p.author_agent_id,
        "content": p.content,
        "metadata": optional_pb_struct_to_json(p.metadata),
        "revision_count": p.revision_count,
    })
}

pub(crate) fn validation_outcome_to_json(v: pb::ValidationOutcome) -> Value {
    let reports: Vec<Value> = v
        .reports
        .into_iter()
        .map(|r| {
            json!({
                "kind": r.kind,
                "passed": r.passed,
                "summary": r.summary,
                "details": optional_pb_struct_to_json(r.details),
            })
        })
        .collect();
    let passed_overall = reports
        .iter()
        .all(|r| r["passed"].as_bool().unwrap_or(false));
    json!({
        "score": v.score,
        "passed": passed_overall,
        "reports": reports,
    })
}

pub(crate) fn deliberation_result_to_json(r: pb::DeliberationResult) -> Value {
    json!({
        "rank": r.rank,
        "proposal": r.proposal.map_or(Value::Null, proposal_to_json),
        "validation": r
            .validation
            .map_or(Value::Null, validation_outcome_to_json),
    })
}

pub(crate) fn deliberate_response_to_json(r: pb::DeliberateResponse) -> Value {
    json!({
        "task_id": r.task_id,
        "winner_proposal_id": r.winner_proposal_id,
        "duration_ms": r.duration_ms,
        "results": r
            .results
            .into_iter()
            .map(deliberation_result_to_json)
            .collect::<Vec<_>>(),
        "metadata": optional_pb_struct_to_json(r.metadata),
    })
}

pub(crate) fn deliberation_update_to_json(u: pb::DeliberationUpdate) -> Value {
    let payload = match u.payload {
        None => Value::Null,
        Some(pb::deliberation_update::Payload::Proposal(p)) => json!({
            "kind": "proposal",
            "proposal": proposal_to_json(p),
        }),
        Some(pb::deliberation_update::Payload::Critique(c)) => json!({
            "kind": "critique",
            "critique": {
                "reviewer_agent_id": c.reviewer_agent_id,
                "target_proposal_id": c.target_proposal_id,
                "feedback": c.feedback,
            }
        }),
        Some(pb::deliberation_update::Payload::Revision(r)) => json!({
            "kind": "revision",
            "revision": {
                "author_agent_id": r.author_agent_id,
                "proposal_id": r.proposal_id,
                "updated_content": r.updated_content,
            }
        }),
        Some(pb::deliberation_update::Payload::Validation(v)) => json!({
            "kind": "validation",
            "validation": validation_outcome_to_json(v),
        }),
        Some(pb::deliberation_update::Payload::Result(r)) => json!({
            "kind": "result",
            "result": deliberation_result_to_json(r),
        }),
    };
    json!({
        "task_id": u.task_id,
        "phase": phase_name(u.phase),
        "emitted_at": timestamp_to_rfc3339(u.emitted_at.as_ref()),
        "payload": payload,
    })
}

pub(crate) fn orchestrate_response_to_json(r: pb::OrchestrateResponse) -> Value {
    json!({
        "task_id": r.task_id,
        "execution_id": r.execution_id,
        "duration_ms": r.duration_ms,
        "winner": r
            .winner
            .map_or(Value::Null, deliberation_result_to_json),
        "candidates": r
            .candidates
            .into_iter()
            .map(deliberation_result_to_json)
            .collect::<Vec<_>>(),
        "metadata": optional_pb_struct_to_json(r.metadata),
    })
}

pub(crate) fn agent_summary_to_json(a: pb::AgentSummary) -> Value {
    json!({
        "agent_id": a.agent_id,
        "specialty": a.specialty,
        "kind": a.kind,
        "attributes": optional_pb_struct_to_json(a.attributes),
    })
}

pub(crate) fn council_summary_to_json(c: pb::CouncilSummary) -> Value {
    json!({
        "specialty": c.specialty,
        "num_agents": c.num_agents,
        "created_at": timestamp_to_rfc3339(c.created_at.as_ref()),
        "agents": c
            .agents
            .into_iter()
            .map(agent_summary_to_json)
            .collect::<Vec<_>>(),
    })
}

pub(crate) fn trigger_ack_to_json(a: &pb::TriggerAck) -> Value {
    json!({
        "event_id": a.event_id,
        "accepted": a.accepted,
        "dispatched_task_ids": a.dispatched_task_ids,
        "reason": a.reason,
    })
}

pub(crate) fn output_contract_to_json(c: pb::OutputContract) -> Value {
    let format = match pb::OutputFormat::try_from(c.format).unwrap_or(pb::OutputFormat::Unspecified)
    {
        pb::OutputFormat::Unspecified | pb::OutputFormat::JsonObject => "json_object",
    };
    let fields: Map<String, Value> = c
        .fields
        .into_iter()
        .map(|(name, rule)| {
            (
                name,
                json!({
                    "required": rule.required,
                    "allowed_string_values": rule.allowed_string_values,
                }),
            )
        })
        .collect();
    json!({
        "contract_id": c.contract_id,
        "format": format,
        "fields": Value::Object(fields),
        "json_schema": c.json_schema,
    })
}

fn validation_mode_name(value: i32) -> &'static str {
    match pb::ValidationMode::try_from(value).unwrap_or(pb::ValidationMode::Unspecified) {
        pb::ValidationMode::Unspecified => "VALIDATION_MODE_UNSPECIFIED",
        pb::ValidationMode::Strict => "VALIDATION_MODE_STRICT",
        pb::ValidationMode::Warn => "VALIDATION_MODE_WARN",
    }
}

fn candidate_summary_to_json(c: pb::CandidateSummary) -> Value {
    json!({
        "proposal_id": c.proposal_id,
        "author_agent_id": c.author_agent_id,
        "score": c.score,
        "reports": c
            .reports
            .into_iter()
            .map(|r| json!({
                "kind": r.kind,
                "passed": r.passed,
                "summary": r.summary,
                "details": optional_pb_struct_to_json(r.details),
            }))
            .collect::<Vec<_>>(),
        "rank": c.rank,
        "passed": c.passed,
        "revision_count": c.revision_count,
    })
}

fn validation_outcome_summary_to_json(v: pb::ValidationOutcomeSummary) -> Value {
    json!({
        "passed": v.passed,
        "candidates_passed": v.candidates_passed,
        "candidates_total": v.candidates_total,
    })
}

pub(crate) fn run_council_decision_response_to_json(r: pb::RunCouncilDecisionResponse) -> Value {
    json!({
        "task_id": r.task_id,
        "winner": r.winner.map_or(Value::Null, deliberation_result_to_json),
        "validation": r
            .validation
            .map_or(Value::Null, validation_outcome_summary_to_json),
        "candidates": r
            .candidates
            .into_iter()
            .map(candidate_summary_to_json)
            .collect::<Vec<_>>(),
        "duration_ms": r.duration_ms,
        "validation_mode": validation_mode_name(r.validation_mode),
    })
}

fn ceremony_step_execution_to_json(step: pb::CeremonyStepExecution) -> Value {
    let pb::CeremonyStepExecution {
        state_id,
        step_id,
        role_id,
        status,
        attempt,
        output,
        iteration,
    } = step;
    json!({
        "state_id": state_id,
        "step_id": step_id,
        "role_id": role_id,
        // The contract spells a step status the way the session view
        // spells it — `completed`, not `COMPLETED`. The proto carries
        // the transport's own spelling, and a client reading a run's
        // trace and then the session's would otherwise be told two
        // different words for one status depending on which backend
        // answered.
        "status": status.to_ascii_lowercase(),
        "attempt": attempt,
        "iteration": iteration,
        "output": output,
    })
}

pub(crate) fn run_ceremony_response_to_json(r: pb::RunCeremonyResponse) -> Value {
    json!({
        "ceremony_id": r.ceremony_id,
        "definition_name": r.definition_name,
        "definition_version": r.definition_version,
        "final_state": r.final_state,
        "completed": r.completed,
        "steps": r
            .steps
            .into_iter()
            .map(ceremony_step_execution_to_json)
            .collect::<Vec<_>>(),
        "mermaid_sequence": r.mermaid_sequence,
    })
}

pub(crate) fn statistics_to_json(s: pb::Statistics) -> Value {
    let per_specialty: Map<String, Value> = s
        .per_specialty_counts
        .into_iter()
        .map(|(k, v)| (k, Value::Number(JsonNumber::from(v))))
        .collect();
    json!({
        "total_deliberations": s.total_deliberations,
        "total_orchestrations": s.total_orchestrations,
        "total_duration_ms": s.total_duration_ms,
        "average_duration_ms": s.average_duration_ms,
        "per_specialty_counts": Value::Object(per_specialty),
    })
}

// ---------------------------------------------------------------------------
// Authoring
// ---------------------------------------------------------------------------

pub(crate) fn validate_ceremony_draft_to_json(
    response: pb::ValidateCeremonyDraftResponse,
) -> Value {
    json!({
        "ceremony": response.ceremony,
        "version": response.version,
        "publishable": response.publishable,
        "error_count": response.error_count,
        "warning_count": response.warning_count,
        "findings": response
            .findings
            .into_iter()
            .map(|finding| json!({
                "severity": finding.severity,
                "locus": finding.locus.map_or(Value::Null, |locus| Value::Object(pb_struct_to_json(locus))),
                "message": finding.message,
            }))
            .collect::<Vec<_>>(),
    })
}

pub(crate) fn explain_ceremony_draft_to_json(response: &pb::ExplainCeremonyDraftResponse) -> Value {
    json!({
        "ceremony": response.ceremony,
        "version": response.version,
        "publishable": response.publishable,
        "summary": response.summary.as_ref().map_or_else(
            || json!({}),
            |summary| json!({
                "states": summary.states,
                "initial_states": summary.initial_states,
                "terminal_states": summary.terminal_states,
                "transitions": summary.transitions,
                "steps": summary.steps,
                "guards": summary.guards,
                "roles": summary.roles,
            }),
        ),
        "narrative": response.narrative.clone(),
    })
}

/// Publishing answers one of three outcomes, and each carries only the
/// fields that mean anything for it. Emitting the others as empty
/// strings would tell a reader that a refused publication has a digest
/// of "".
pub(crate) fn publish_ceremony_definition_to_json(
    response: &pb::PublishCeremonyDefinitionResponse,
) -> Value {
    if response.outcome == "version_occupied" {
        json!({
            "outcome": response.outcome,
            "published_digest": response.published_digest,
            "offered_digest": response.offered_digest,
        })
    } else {
        json!({
            "outcome": response.outcome,
            "ceremony": response.ceremony,
            "version": response.version,
            "digest": response.digest,
        })
    }
}

pub(crate) fn diff_ceremony_definitions_to_json(
    response: pb::DiffCeremonyDefinitionsResponse,
) -> Value {
    json!({
        "identical": response.identical,
        "strands_running_sessions": response.strands_running_sessions,
        "strand_count": response.strand_count,
        "changes": response
            .changes
            .into_iter()
            .map(|change| json!({
                "kind": change.kind,
                "locus": change.locus.map_or(Value::Null, |locus| Value::Object(pb_struct_to_json(locus))),
                "impact": change.impact,
                "detail": change.detail,
            }))
            .collect::<Vec<_>>(),
    })
}
