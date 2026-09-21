//! The ordered routes both audiences are offered.
//!
//! Its own file rather than a list inside `guidance.rs`, which already
//! assembles two whole help documents: a workflow is a sequence and a
//! reason, and every one of them is filtered by the same rule — a route
//! whose tools this backend does not all serve is not offered at all,
//! because a half-available sequence is worse guidance than none.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::protocol::{
    ACKNOWLEDGE_CEREMONY_AGENT_INTERVENTION_TOOL, ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL,
    APPLY_CEREMONY_TRANSITION_TOOL, DESIGN_AGENTIC_SYSTEM_TOOL, DESIGN_CEREMONY_TOOL,
    DISCOVER_CAPABILITIES_TOOL, EXPLAIN_CEREMONY_DRAFT_TOOL, GENERATE_CEREMONY_REPORT_TOOL,
    GET_CEREMONY_INSTANCE_TOOL, GET_CEREMONY_INTERVENTION_TOOL, GET_CEREMONY_TRANSCRIPT_TOOL,
    INSTANTIATE_AGENTIC_SYSTEM_TOOL, LIST_CEREMONY_INSTANCES_TOOL, PAUSE_CEREMONY_TOOL,
    PLAN_CEREMONY_SUCCESSOR_TOOL, PUBLISH_AGENTIC_SYSTEM_TOOL, PUBLISH_CEREMONY_DEFINITION_TOOL,
    PULL_CEREMONY_AGENT_INTERVENTIONS_TOOL, READ_CEREMONY_EVENTS_TOOL,
    REQUEST_CEREMONY_INTERVENTION_TOOL, RESPOND_TO_CEREMONY_INTERVENTION_TOOL,
    RUN_CEREMONY_STEP_TOOL, RUN_CEREMONY_TOOL, START_CEREMONY_SUCCESSOR_TOOL, START_CEREMONY_TOOL,
    VALIDATE_AGENTIC_SYSTEM_TOOL, VALIDATE_CEREMONY_DRAFT_TOOL, VERIFY_CEREMONY_JOURNAL_TOOL,
};

/// Every route whose tools the active catalog serves end to end.
///
/// A route is offered only when this backend serves every tool it
/// names: half a sequence is worse guidance than none.
pub(super) fn available_workflows(names: &BTreeSet<String>) -> Vec<Value> {
    writing_routes()
        .into_iter()
        .chain(running_routes())
        .filter(|workflow| workflow_tools(workflow).all(|tool| names.contains(tool)))
        .collect()
}

/// Finding out what this build serves, and writing down what to run.
fn writing_routes() -> Vec<Value> {
    vec![

        workflow(
            "inspect_available_capabilities",
            "See what this server can actually do",
            "Start here when backend, version, or installed plugin surface is uncertain.",
            &[(
                DISCOVER_CAPABILITIES_TOOL,
                "Read the active version, backend, tools, capability groups, declared limits and generators.",
            )],
        ),
        workflow(
            "design_review_and_publish",
            "Design and review a ceremony",
            "Create an unpublished draft, explain it, validate it, then publish only on explicit request.",
            &[
                (DESIGN_CEREMONY_TOOL, "Create an analysed, unpublished draft."),
                (EXPLAIN_CEREMONY_DRAFT_TOOL, "Read back its declared behavior and blockers."),
                (VALIDATE_CEREMONY_DRAFT_TOOL, "Verify the exact YAML before publication."),
                (PUBLISH_CEREMONY_DEFINITION_TOOL, "Publish only after the reviewed version is authorized."),
            ],
        ),
        workflow(
            "compose_a_system_of_ceremonies",
            "Design a system of several ceremonies",
            "The level above one ceremony: roles, participants and several published ceremonies composed together. Validate before sealing, and seal the revision you read. Authorization for all of it is global: there is no per-system scope.",
            &[
                (DESIGN_AGENTIC_SYSTEM_TOOL, "Write the system down against the revision you read."),
                (VALIDATE_AGENTIC_SYSTEM_TOOL, "Resolve every pin and read every finding at once."),
                (PUBLISH_AGENTIC_SYSTEM_TOOL, "Seal that revision so a run can pin it."),
                (INSTANTIATE_AGENTIC_SYSTEM_TOOL, "Open a run, offering what your host can actually supply."),
                (ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL, "Start what is now ready; repeat as instances complete."),
            ],
        ),
    ]
}

/// Running it, asking about it, revising it and reading it back.
fn running_routes() -> Vec<Value> {
    vec![
        workflow(
            "run_one_shot",
            "Run a ceremony to completion",
            "Use only when no later human decision or delegated host work must pause execution.",
            &[(RUN_CEREMONY_TOOL, "Run the supplied YAML and inspect completed plus step results.")],
        ),
        workflow(
            "drive_durable_instance",
            "Drive a persistent ceremony incrementally",
            "Start, inspect, execute one declared step, and apply only an enabled transition.",
            &[
                (START_CEREMONY_TOOL, "Start without advancing."),
                (GET_CEREMONY_INSTANCE_TOOL, "Inspect current state and the exact next action."),
                (RUN_CEREMONY_STEP_TOOL, "Persist one declared step result."),
                (APPLY_CEREMONY_TRANSITION_TOOL, "Apply an enabled transition."),
            ],
        ),
        workflow(
            "put_a_question_to_a_working_agent",
            "Ask a working agent something, and know it arrived",
            "An intervention aimed at the execution that is running, taken under a lease and acknowledged by name. The roster is process-local: the agent pulls from the session that reported its status.",
            &[
                (REQUEST_CEREMONY_INTERVENTION_TOOL, "Ask, naming the target execution, its incarnation and the seat it holds."),
                (PULL_CEREMONY_AGENT_INTERVENTIONS_TOOL, "From the working agent, take your own questions under an expiring lease."),
                (ACKNOWLEDGE_CEREMONY_AGENT_INTERVENTION_TOOL, "Say what you saw: `received`, `refused` or `incapable` is sealed; `busy` and `timeout` put it back."),
                (RESPOND_TO_CEREMONY_INTERVENTION_TOOL, "Answer, naming the delivery you were handed."),
                (GET_CEREMONY_INTERVENTION_TOOL, "Read the projected status and every route the item took."),
            ],
        ),
        workflow(
            "hand_off_to_a_successor",
            "Revise a definition a paused ceremony cannot finish under",
            "Read the whole plan before sealing anything. Completed work is carried by reference; the budget always starts fresh, and `transfer_remaining` is refused.",
            &[
                (PAUSE_CEREMONY_TOOL, "Pause the ceremony whose definition turned out to be wrong."),
                (PLAN_CEREMONY_SUCCESSOR_TOOL, "Read the diff, the preflight, the evidence that would carry, every claim's required disposition and the blockers. This seals nothing."),
                (START_CEREMONY_SUCCESSOR_TOOL, "Seal the handoff and open the successor under your own `plan_id`; an identical retry verifies rather than duplicates."),
                (GET_CEREMONY_INSTANCE_TOOL, "Read the successor; the predecessor can now only be cancelled."),
            ],
        ),
        workflow(
            "resume_after_context_loss",
            "Resume an existing ceremony",
            "Rediscover backend-owned instances before creating a replacement.",
            &[
                (LIST_CEREMONY_INSTANCES_TOOL, "List known instances."),
                (GET_CEREMONY_INSTANCE_TOOL, "Refresh the selected instance."),
            ],
        ),
        workflow(
            "inspect_ceremony_history",
            "Read what a ceremony recorded",
            "Read the sealed records the session produced and the contributions its steps made, and check that the chain sealing them holds. Nothing here changes anything.",
            &[
                (READ_CEREMONY_EVENTS_TOOL, "Read the stream from the version you have already seen; the answer says where to continue."),
                (VERIFY_CEREMONY_JOURNAL_TOOL, "Check the chain before quoting the stream as evidence; a break names the first position that cannot be trusted."),
                (GET_CEREMONY_TRANSCRIPT_TOOL, "Read the ordered contributions the steps produced."),
            ],
        ),
        workflow(
            "generate_report",
            "Generate a ceremony report",
            "Select persisted instances and project their state plus journal into deterministic Markdown.",
            &[
                (LIST_CEREMONY_INSTANCES_TOOL, "Find the exact ceremony ids to report."),
                (GENERATE_CEREMONY_REPORT_TOOL, "Generate Markdown without writing a file."),
            ],
        ),
    ]
}

fn workflow(id: &str, title: &str, summary: &str, steps: &[(&str, &str)]) -> Value {
    json!({
        "id": id,
        "title": title,
        "summary": summary,
        "steps": steps
            .iter()
            .enumerate()
            .map(|(index, (tool, purpose))| json!({
                "order": index + 1,
                "tool": tool,
                "purpose": purpose,
            }))
            .collect::<Vec<_>>(),
    })
}

/// The tools a workflow names, which is also what decides whether it is offered.
pub(super) fn workflow_tools(workflow: &Value) -> impl Iterator<Item = &str> {
    workflow["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|step| step["tool"].as_str())
}
