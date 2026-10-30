//! Fixture-backed [`MadeMcpToolBackend`].
//!
//! Returns canned, deterministic responses for every tool. Useful for:
//!
//! - validating client wiring (Claude Desktop / Codex CLI) without a
//!   running MADE in reach;
//! - smoke tests in repos that consume this crate;
//! - documenting the expected response shape — the fixtures are the
//!   reference JSON that real gRPC responses will mirror field-for-field.

use async_trait::async_trait;
use protocol_instance_fixture::protocol_instance_fixture;
use serde_json::{json, Value};

use crate::backend::{MadeMcpToolBackend, MadeMcpToolFuture};

mod artifact_fixtures;
mod authorization_fixtures;
mod children_fixtures;
mod council_fixtures;
mod protocol_agent_fixtures;
mod protocol_history_fixtures;
use council_fixtures::{
    deliberate_fixture, get_deliberation_fixture, orchestrate_fixture, stream_fixture,
};
mod agentic_system_fixtures;
mod delivery_fixtures;
mod protocol_instance_fixture;
use agentic_system_fixtures::{handles as is_agentic_system, response as agentic_system};
use artifact_fixtures::{handles as is_artifact, response as artifact};
mod definition_catalogue_fixtures;
mod execution_receipt_fixtures;
mod host_handoff_fixtures;
mod integrator_loop_fixtures;
mod succession_fixtures;

use crate::renderers::{
    ProtocolInstanceListing, ProtocolInstanceListingEntry, ProtocolInstanceSearchPage,
    ServiceMetricsView, StatisticsView,
};
use children_fixtures::{
    accept_child_completion_fixture, prepare_protocol_children_fixture,
    recover_protocol_children_fixture,
};
use protocol_history_fixtures::{
    protocol_report_fixture, protocol_transcript_fixture, pull_protocol_events_fixture,
    read_protocol_events_fixture, stream_protocol_fixture, verify_protocol_journal_fixture,
};

/// Backend that returns canned JSON for every tool. The shapes are
/// kept aligned with what [`crate::grpc::GrpcMadeMcpBackend`] will
/// emit in live mode so client wiring is identical across modes.
#[derive(Debug, Default, Clone)]
pub struct FixtureMadeMcpBackend;

#[async_trait]
impl MadeMcpToolBackend for FixtureMadeMcpBackend {
    fn backend_name(&self) -> &'static str {
        "fixture"
    }

    fn supports_tool(&self, name: &str) -> bool {
        crate::mcp_protocol::is_grpc_tool(name)
    }

    // One arm per tool, even where the canned answer is the same:
    // `grpc_dispatch_and_fixture_cover_every_catalog_tool` reads this
    // file to check that no tool is missing a response, and a merged
    // arm would hide the ones folded into it.
    #[allow(clippy::match_same_arms, clippy::too_many_lines)]
    fn call_tool<'a>(&'a self, name: &'a str, _arguments: &'a Value) -> MadeMcpToolFuture<'a> {
        Box::pin(async move {
            let structured = match name {
                "made_read_council_events" => json!({"records":[],"next_after":null}),
                "made_get_council_event_cursor" => json!({"acknowledged_through":null}),
                "made_lease_council_events" => json!({"lease":null}),
                "made_acknowledge_council_events" => json!({}),
                "made_release_council_events" => json!({}),
                "made_deliberate" => deliberate_fixture(),
                "made_stream_deliberation" => stream_fixture(),
                "made_get_deliberation_result" => get_deliberation_fixture(),
                "made_orchestrate" => orchestrate_fixture(),
                "made_create_council" => create_council_fixture(),
                "made_list_councils" => list_councils_fixture(),
                "made_delete_council" => delete_council_fixture(),
                "made_register_agent" => register_agent_fixture(),
                "made_unregister_agent" => unregister_agent_fixture(),
                "made_process_trigger_event" => process_trigger_fixture(),
                "made_run_council_decision" => run_council_decision_fixture(),
                "made_register_contract" => register_contract_fixture(),
                "made_list_contracts" => list_contracts_fixture(),
                "made_delete_contract" => delete_contract_fixture(),
                "made_run_protocol" => run_protocol_fixture(),
                "made_get_protocol_instance" => protocol_instance_fixture(),
                "made_list_protocol_agents" => protocol_agent_fixtures::list(),
                "made_get_protocol_agent" => protocol_agent_fixtures::one(),
                "made_report_protocol_agent_status" => protocol_agent_fixtures::one(),
                "made_start_protocol" => protocol_instance_fixture(),
                "made_start_published_protocol" => protocol_instance_fixture(),
                "made_run_protocol_step" => protocol_instance_fixture(),
                "made_prepare_protocol_children" => prepare_protocol_children_fixture(),
                "made_accept_child_completion" => accept_child_completion_fixture(),
                "made_recover_protocol_children" => recover_protocol_children_fixture(),
                "made_apply_protocol_transition" => protocol_instance_fixture(),
                "made_pause_protocol" => protocol_instance_fixture(),
                "made_resume_protocol" => protocol_instance_fixture(),
                name if is_agentic_system(name) => agentic_system(name),
                "made_record_protocol_host_handoff" => host_handoff_fixtures::response(name),
                "made_inspect_protocol_resume" => host_handoff_fixtures::response(name),
                "made_plan_protocol_successor" => succession_fixtures::response(name),
                "made_start_protocol_successor" => succession_fixtures::response(name),
                "made_cancel_protocol" => protocol_instance_fixture(),
                "made_enforce_protocol_deadlines" => protocol_instance_fixture(),
                "made_approve_protocol_guard" => protocol_instance_fixture(),
                "made_defer_protocol_guard" => protocol_instance_fixture(),
                "made_request_protocol_intervention" => protocol_instance_fixture(),
                "made_respond_to_protocol_intervention" => protocol_instance_fixture(),
                "made_close_protocol_intervention" => protocol_instance_fixture(),
                name if delivery_fixtures::handles(name) => delivery_fixtures::response(name),
                name if integrator_loop_fixtures::handles(name) => {
                    integrator_loop_fixtures::response(name)
                }
                "made_collect_protocol_evidence" => protocol_instance_fixture(),
                "made_assert_protocol_reason" => protocol_instance_fixture(),
                "made_list_protocol_instances" => protocol_listing_fixture(),
                "made_search_protocol_instances" => protocol_search_fixture(),
                "made_design_protocol" => design_protocol_fixture(),
                "made_read_protocol_events" => read_protocol_events_fixture(),
                "made_stream_protocol" => stream_protocol_fixture(),
                "made_pull_protocol_events" => pull_protocol_events_fixture(),
                "made_verify_protocol_journal" => verify_protocol_journal_fixture(),
                "made_get_protocol_transcript" => protocol_transcript_fixture(),
                "made_generate_protocol_report" => protocol_report_fixture(),
                "made_get_budget_report" => json!({
                    "account_id": "fixture-budget",
                    "balance": {
                        "limits": {"duration_micros": null, "tokens": 100, "cost_micros": null, "tool_calls": null, "currency": null},
                        "reserved": {"duration_micros": 0, "tokens": 20, "cost_micros": 0, "tool_calls": 0},
                        "observed": {"duration_micros": 0, "tokens": 0, "cost_micros": 0, "tool_calls": 0},
                        "estimated": {"duration_micros": 0, "tokens": 0, "cost_micros": 0, "tool_calls": 0},
                        "unconfirmed": {"duration_micros": 0, "tokens": 0, "cost_micros": 0, "tool_calls": 0},
                        "overrun": {"duration_micros": 0, "tokens": 0, "cost_micros": 0, "tool_calls": 0},
                        "available": {"duration_micros": 0, "tokens": 80, "cost_micros": 0, "tool_calls": 0}
                    }
                }),
                "made_list_pending_budget_reservations" => json!({
                    "reservations": [], "next_cursor": null
                }),
                name if is_artifact(name) => artifact(name),
                "made_validate_protocol_draft" => validate_draft_fixture(),
                "made_explain_protocol_draft" => explain_draft_fixture(),
                "made_publish_protocol_definition" => publish_definition_fixture(),
                "made_diff_protocol_definitions" => diff_definitions_fixture(),
                "made_list_protocol_definitions" => definition_catalogue_fixtures::response(name),
                "made_get_protocol_definition" => definition_catalogue_fixtures::response(name),
                "made_bind_protocol_participants" => protocol_instance_fixture(),
                "made_claim_protocol_step" => protocol_instance_fixture(),
                "made_complete_protocol_step" => protocol_instance_fixture(),
                "made_renew_protocol_step_lease" => renewal_fixture(),
                "made_get_execution_receipt" => execution_receipt_fixtures::receipt(),
                "made_inspect_execution_recovery" => execution_receipt_fixtures::recovery_page(),
                "made_complete_execution_receipt" => protocol_instance_fixture(),
                "made_adopt_execution_receipt" => protocol_instance_fixture(),
                "made_get_authorization_policy" => authorization_policy_fixture(),
                "made_issue_authorization_grant" => json!({"version":2,"existing":false}),
                "made_revoke_authorization_grant" => json!({"version":3,"existing":false}),
                "made_approve_authorization_operation" => authorization_fixtures::approval(),
                "made_list_authorization_decisions" => authorization_fixtures::decisions(),
                "made_get_status" => get_status_fixture(),
                "made_get_metrics" => get_metrics_fixture(),
                other => {
                    return Err(crate::mcp_protocol::ToolError::invalid_request(format!(
                        "fixture backend: unknown tool `{other}` (this is a client-side typo, not a backend error)"
                    )));
                }
            };
            Ok(crate::mcp_protocol::tool_success_result(structured))
        })
    }
}

// ---------------------------------------------------------------------------
// Canned fixtures. Stable across releases so client wiring stays
// reproducible. New fields appear here first, then in the real
// adapter; tests in `tests/stdio_protocol.rs` pin the shape.
// ---------------------------------------------------------------------------

fn create_council_fixture() -> Value {
    json!({
        "council": {
            "specialty": "triage",
            "num_agents": 1,
            "created_at": null,
            "agents": []
        }
    })
}

fn list_councils_fixture() -> Value {
    json!({
        "councils": [
            { "specialty": "triage", "num_agents": 1, "created_at": null, "agents": [] }
        ]
    })
}

fn delete_council_fixture() -> Value {
    json!({ "deleted": true })
}

fn register_agent_fixture() -> Value {
    json!({ "agent_id": "agent-fixture-1" })
}

fn unregister_agent_fixture() -> Value {
    json!({ "unregistered": true })
}

fn process_trigger_fixture() -> Value {
    json!({
        "ack": {
            "event_id": "evt-fixture-1",
            "accepted": true,
            "dispatched_task_ids": ["task-fixture-1"],
            "reason": ""
        }
    })
}

fn get_status_fixture() -> Value {
    json!({
        "version": "fixture",
        "uptime_seconds": 0,
        "health": "healthy",
        "stats": null
    })
}

fn get_metrics_fixture() -> Value {
    ServiceMetricsView {
        statistics: Some(StatisticsView::default()),
        registry_text: String::new(),
        registry: Vec::new(),
    }
    .to_json()
}

fn run_council_decision_fixture() -> Value {
    json!({
        "task_id": "task-fixture-1",
        "winner": deliberate_fixture()["results"][0].clone(),
        "validation": {
            "passed": true,
            "candidates_passed": 1,
            "candidates_total": 1
        },
        "candidates": [
            {
                "proposal_id": "proposal-fixture-a",
                "author_agent_id": "agent-fixture-1",
                "score": 1.0,
                "reports": [
                    { "kind": "content-non-empty", "passed": true, "summary": "ok", "details": {} }
                ],
                "rank": 0,
                "passed": true,
                "revision_count": 0
            }
        ],
        "duration_ms": 42,
        "validation_mode": "VALIDATION_MODE_STRICT"
    })
}

fn register_contract_fixture() -> Value {
    json!({ "contract_id": "contract-fixture-1" })
}

fn list_contracts_fixture() -> Value {
    json!({
        "contracts": [
            {
                "contract_id": "contract-fixture-1",
                "format": "json_object",
                "fields": {},
                "json_schema": ""
            }
        ]
    })
}

fn delete_contract_fixture() -> Value {
    json!({ "deleted": true })
}

fn run_protocol_fixture() -> Value {
    json!({
        "protocol_id": "protocol-fixture-1",
        "definition_name": "fixture_protocol",
        "definition_version": "1.0",
        "final_state": "CLOSED",
        "completed": true,
        "steps": [
            {
                "state_id": "OPEN",
                "step_id": "collect_context",
                "role_id": "FACILITATOR",
                "status": "COMPLETED",
                "attempt": 1,
                "output": "context collected: the team agreed on the brief"
            }
        ],
        "mermaid_sequence": "sequenceDiagram\n    FACILITATOR->>FACILITATOR: collect_context [noop_step]"
    })
}

/// A listing with both kinds of entry a live engine answers with: a
/// session that could be read, and one whose definition this store does
/// not hold. Both carry `rehydratable` and `reason`, so a client wiring
/// against the fixture meets the shape either backend renders.
fn protocol_listing_fixture() -> Value {
    ProtocolInstanceListing::new(vec![
        ProtocolInstanceListingEntry::rehydratable(protocol_instance_fixture()),
        ProtocolInstanceListingEntry::unrehydratable(
            "protocol-fixture-2",
            "not found: protocol_definition",
        ),
    ])
    .to_json()
}

fn protocol_search_fixture() -> Value {
    ProtocolInstanceSearchPage::new(
        vec![ProtocolInstanceListingEntry::rehydratable(
            protocol_instance_fixture(),
        )],
        Some("fixture-next-cursor".to_owned()),
    )
    .to_json()
}

/// A design that worked: the document it rendered, what it did with
/// the intent, and a clean analysis. Publishable, unlike the draft the
/// validation fixture answers with, because a designer that handed
/// back something unpublishable would be broken.
fn design_protocol_fixture() -> Value {
    json!({
        "protocol": "fixture_protocol",
        "version": "1.0",
        "definition_yaml": FIXTURE_DESIGNED_YAML,
        "publishable": true,
        "design": {
            "topology": "linear",
            "stages": 1,
            "participants": 1,
            "final_approval_required": false
        },
        "analysis": {
            "protocol": "fixture_protocol",
            "version": "1.0",
            "publishable": true,
            "error_count": 0,
            "warning_count": 0,
            "findings": []
        },
        "published": false,
        "started": false
    })
}

const FIXTURE_DESIGNED_YAML: &str = r"version: '1.0'
name: fixture_protocol
description: Decide the fixture question.
inputs:
  required: []
  optional: []
outputs:
  decision:
    type: object
states:
- id: DECIDE
  initial: true
- id: COMPLETED
  terminal: true
transitions:
- from: DECIDE
  to: COMPLETED
  trigger: decide_completed
  guards:
  - decide_completed
steps:
- id: decide
  state: DECIDE
  handler: host_callback
  config:
    num_agents: 1
    prompt: Decide the fixture question.
    see_prior: false
guards:
  decide_completed:
    type: automated
    check: step_status:decide:COMPLETED
roles:
- id: DECIDER
  allowed_actions:
  - decide
  - decide_completed
timeouts:
  step_default: 300
retry_policies:
  default:
    max_attempts: 2
    backoff_seconds: 1
";

fn validate_draft_fixture() -> Value {
    json!({
        "protocol": "fixture_protocol",
        "version": "1.0",
        "publishable": false,
        "error_count": 1,
        "warning_count": 0,
        "findings": [
            {
                "severity": "error",
                "locus": { "kind": "transition", "from": "OPEN", "trigger": "finish" },
                "message": "not found: protocol_transition.to_state"
            }
        ]
    })
}

fn explain_draft_fixture() -> Value {
    json!({
        "protocol": "fixture_protocol",
        "version": "1.0",
        "publishable": false,
        "summary": {
            "states": 2,
            "initial_states": 1,
            "terminal_states": 1,
            "transitions": 1,
            "steps": 1,
            "guards": 1,
            "roles": 1
        },
        "narrative": [
            "`fixture_protocol` declares 2 states, 1 transitions, 1 steps, 1 guards and 1 roles.",
            "1 defect(s) block publication; the draft cannot be published or executed until every one is fixed.",
            "not found: protocol_transition.to_state — at transition `finish` out of state `OPEN`"
        ]
    })
}

fn diff_definitions_fixture() -> Value {
    json!({
        "identical": false,
        "strands_running_sessions": true,
        "strand_count": 1,
        "changes": [
            {
                "kind": "removed",
                "locus": { "kind": "state", "state": "REVIEW" },
                "impact": "strands",
                "detail": "a session in this state would have nowhere to be"
            },
            {
                "kind": "added",
                "locus": { "kind": "role", "role": "OBSERVER" },
                "impact": "carries",
                "detail": "another role at the table"
            }
        ]
    })
}

fn publish_definition_fixture() -> Value {
    json!({
        "outcome": "published",
        "protocol": "fixture_protocol",
        "version": "1.0",
        "digest": "3f786850e387550fdab836ed7e6dc881de23001b"
    })
}

fn authorization_policy_fixture() -> Value {
    json!({"policy":{
        "policy_id":"fixture-policy","version":1,
        "owner":{"principal_id":"fixture-host","kind":"trusted_host","authentication_method":"local_host_policy"},
        "grants":[],"revocations":[],"separation_rules":[]
    }})
}

fn renewal_fixture() -> Value {
    json!({"renewal_id":"fixture-heartbeat", "claim_fence":"fixture-fence",
        "effective_lease_expires_at":"2026-01-01T01:00:00Z", "renewed_at":"2026-01-01T00:30:00Z"})
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn unknown_tool_returns_explicit_error() {
        let backend = FixtureMadeMcpBackend;
        let err = backend.call_tool("nope", &json!({})).await.unwrap_err();
        assert!(err.message().contains("unknown tool"));
        assert_eq!(
            err.code(),
            crate::mcp_protocol::ToolErrorCode::InvalidRequest
        );
    }

    #[tokio::test]
    async fn deliberate_returns_mcp_success_envelope() {
        let backend = FixtureMadeMcpBackend;
        let v = backend
            .call_tool("made_deliberate", &json!({}))
            .await
            .unwrap();
        assert_eq!(v["isError"], false);
        assert_eq!(
            v["structuredContent"]["winner_proposal_id"],
            "proposal-fixture-a"
        );
    }

    #[tokio::test]
    async fn every_backend_tool_has_a_fixture() {
        let backend = FixtureMadeMcpBackend;
        let catalog = crate::mcp_protocol::tools_list_result(|name| backend.supports_tool(name));
        for tool in catalog["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
        {
            if crate::mcp_protocol::is_server_tool(tool) {
                continue;
            }
            let v = backend.call_tool(tool, &json!({})).await.unwrap();
            assert_eq!(v["isError"], false, "{tool} fixture must be a success");
        }
    }
}
