//! What this release does not do, said out loud where a host looks.
//!
//! A limit that lives only in a release note is a limit a host meets as
//! an error it cannot explain. Each entry names the capability group it
//! belongs to, the boundary, and what to do instead, and each is gated
//! on the tools that reach it: a backend that does not serve the group
//! does not declare its limits either.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::protocol::{
    ACKNOWLEDGE_CEREMONY_AGENT_INTERVENTION_TOOL, AWAIT_INTEGRATOR_ATTENTION_TOOL,
    DESIGN_AGENTIC_SYSTEM_TOOL, LIST_CEREMONY_INTERVENTIONS_TOOL, PLAN_CEREMONY_SUCCESSOR_TOOL,
    PULL_CEREMONY_AGENT_INTERVENTIONS_TOOL, START_CEREMONY_SUCCESSOR_TOOL,
};

/// Every declared limit the active catalog can actually run into.
pub(super) fn declared_limits(names: &BTreeSet<String>) -> Vec<Value> {
    let mut limits = Vec::new();
    if names.contains(PLAN_CEREMONY_SUCCESSOR_TOOL) || names.contains(START_CEREMONY_SUCCESSOR_TOOL)
    {
        limits.push(json!({
            "id": "successor_budget_transfer_is_refused",
            "capability": "ceremony_recovery",
            "limit": "A successor always opens on a fresh budget account. `budget: transfer_remaining` is spelled in the contract so it can be refused with a reason, never quietly downgraded to `fresh`.",
            "because": "Reservations, root accounting and refunds are held per ceremony; moving a balance without its reservation ledger would leave two ceremonies believing they hold it.",
            "instead": "Plan the successor with `budget: fresh` and reserve again against the successor's own account.",
        }));
    }
    if names.contains(DESIGN_AGENTIC_SYSTEM_TOOL) {
        limits.push(json!({
            "id": "agentic_system_has_no_authorization_scope",
            "capability": "agentic_system_design",
            "limit": "Operations on the aggregate and on its executions are authorized at `global` scope. There is no `agentic_system` authorization scope, so a grant cannot be narrowed to one system or one run.",
            "because": "The scope is a value in the authorization contract; adding one is a contract change, and this release keeps the existing values.",
            "instead": "Issue the grant to a grantee that only designs and runs systems, rather than widening a grant a ceremony host already holds.",
        }));
    }
    if names.contains(PULL_CEREMONY_AGENT_INTERVENTIONS_TOOL)
        && names.contains(ACKNOWLEDGE_CEREMONY_AGENT_INTERVENTION_TOOL)
    {
        limits.push(json!({
            "id": "agent_roster_is_process_local",
            "capability": "ceremony_participation",
            "limit": "The live agent roster is held by the process that was told about it. An agent pulls its own questions from the same server session that reported its status; another process sees no such execution.",
            "because": "Status is bounded live telemetry, not a sealed record. What crosses processes is the durable delivery ledger, not the roster.",
            "instead": format!("Report status and pull from one long-lived host session. A question aimed at an execution is also routed to its seat, so {LIST_CEREMONY_INTERVENTIONS_TOOL} finds it from anywhere."),
        }));
    }
    if names.contains(AWAIT_INTEGRATOR_ATTENTION_TOOL) {
        limits.push(json!({
            "id": "host_activation_is_not_reported_over_grpc",
            "capability": "integrator_loop",
            "limit": "Through the gRPC-backed MCP server `host_activation.adapter` reads `none` whatever the service composed. The embedded backend and the Rust facade answer from the engine they are holding.",
            "because": "The adapter is a fact about the service's own process, and the versioned contract carries no field to forward it.",
            "instead": format!("Over gRPC, read `none` as unknown rather than as configured, and follow the scope with {AWAIT_INTEGRATOR_ATTENTION_TOOL} instead of waiting to be woken."),
        }));
    }
    limits
}
