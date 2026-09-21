//! Server-owned discovery and audience guidance.
//!
//! These responses are projected from the same backend-filtered catalog used
//! by MCP `tools/list`. They therefore describe the server that answered the
//! call, rather than a separately maintained promise about some other build.

use authority_boundaries::base_agent_authority_boundaries;
use base_agent_preconditions::base_agent_preconditions;
use std::collections::BTreeSet;
use std::fmt::Write as _;

use made_core::value_objects::HostActivationAdapterKind;
use serde_json::{json, Map, Value};

use crate::mcp_server_identity::McpServerIdentity;
use crate::protocol::{
    available_tool_catalog, design_pattern_catalog, ADOPT_EXECUTION_RECEIPT_TOOL,
    ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL, APPLY_CEREMONY_TRANSITION_TOOL,
    CLAIM_CEREMONY_STEP_TOOL, COMPLETE_CEREMONY_STEP_TOOL, COMPLETE_EXECUTION_RECEIPT_TOOL,
    DESIGN_AGENTIC_SYSTEM_TOOL, DESIGN_CEREMONY_TOOL, DISCOVER_CAPABILITIES_TOOL,
    EXPLAIN_CEREMONY_DRAFT_TOOL, GENERATE_CEREMONY_REPORT_TOOL, GET_CEREMONY_AGENT_TOOL,
    GET_CEREMONY_INSTANCE_TOOL, GET_CEREMONY_TRANSCRIPT_TOOL, GET_HELP_TOOL,
    INSPECT_EXECUTION_RECOVERY_TOOL, INSTANTIATE_AGENTIC_SYSTEM_TOOL, LIST_CEREMONY_AGENTS_TOOL,
    LIST_CEREMONY_INSTANCES_TOOL, PUBLISH_AGENTIC_SYSTEM_TOOL, PUBLISH_CEREMONY_DEFINITION_TOOL,
    READ_CEREMONY_EVENTS_TOOL, REPORT_CEREMONY_AGENT_STATUS_TOOL, RUN_CEREMONY_STEP_TOOL,
    RUN_CEREMONY_TOOL, START_CEREMONY_TOOL, VALIDATE_AGENTIC_SYSTEM_TOOL,
    VALIDATE_CEREMONY_DRAFT_TOOL, VERIFY_CEREMONY_JOURNAL_TOOL,
};

mod authority_boundaries;
mod base_agent_preconditions;
pub(crate) mod capability_group;
mod delegated_host_sequence;
mod host_activation;
mod integrator_loop_sequence;

use capability_group::CAPABILITY_GROUPS;
use delegated_host_sequence::delegated_host_sequence;
use host_activation::host_activation;
use integrator_loop_sequence::integrator_loop_guidance;

const SCHEMA_VERSION: &str = "1.0";

/// Describe exactly the tools executable through the active server.
pub(crate) fn discovery_result(
    identity: McpServerIdentity,
    backend: &str,
    grpc_tls: &str,
    activation: HostActivationAdapterKind,
    arguments: &Value,
    supports: impl Fn(&str) -> bool,
) -> Result<Value, String> {
    match arguments {
        Value::Null => {}
        Value::Object(object) if object.is_empty() => {}
        Value::Object(object) => {
            return Err(format!(
                "made_discover_capabilities accepts no fields, got: {}",
                object.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        _ => {
            return Err("made_discover_capabilities arguments must be an object".to_owned());
        }
    }
    let catalog = available_tool_catalog(supports);
    let names = catalog_names(&catalog);
    let tools = catalog
        .into_iter()
        .map(|tool| {
            let name = tool["name"].as_str().expect("catalog tools have names");
            json!({
                "name": name,
                "description": tool["description"],
                "input_schema": tool["inputSchema"],
                "report_generator": name == GENERATE_CEREMONY_REPORT_TOOL,
            })
        })
        .collect::<Vec<_>>();
    let artifact_generators = if names.contains(GENERATE_CEREMONY_REPORT_TOOL) {
        vec![json!({
            "id": "ceremony_report_markdown",
            "tool": GENERATE_CEREMONY_REPORT_TOOL,
            "artifact_kind": "ceremony_report",
            "media_type": "text/markdown",
            "response_field": "structuredContent.report_markdown",
            "persisted_by_tool": false,
            "persistence_owner": "host",
        })]
    } else {
        Vec::new()
    };
    let design_patterns = if names.contains(DESIGN_CEREMONY_TOOL) {
        design_pattern_catalog()
    } else {
        Vec::new()
    };

    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "server": {
            "name": identity.name(),
            "version": identity.version(),
        },
        "backend": {
            "name": backend,
            "grpc_tls": grpc_tls,
            "state_durability": "backend_defined",
        },
        "tool_count": tools.len(),
        "capabilities": capability_groups(&names),
        "host_activation": host_activation(&names, activation),
        "artifact_generators": artifact_generators,
        "design_patterns": design_patterns,
        "help": {
            "tool": GET_HELP_TOOL,
            "audiences": ["user", "agent"],
        },
        "tools": tools,
    }))
}

/// Render help for a person or for an autonomous host/agent.
pub(crate) fn help_result(
    arguments: &Value,
    supports: impl Fn(&str) -> bool,
) -> Result<Value, String> {
    let audience = parse_audience(arguments)?;
    let catalog = available_tool_catalog(supports);
    let names = catalog_names(&catalog);
    let workflows = available_workflows(&names);

    let mut help = match audience {
        HelpAudience::User => user_help(&workflows, &names),
        HelpAudience::Agent => agent_help(&workflows, &names),
    };
    let markdown = render_help_markdown(&help);
    help.as_object_mut()
        .expect("help projection is an object")
        .insert("help_markdown".to_owned(), Value::String(markdown));
    Ok(help)
}

fn capability_groups(names: &BTreeSet<String>) -> Vec<Value> {
    CAPABILITY_GROUPS
        .iter()
        .filter_map(|group| {
            let available_tools = group
                .tools
                .iter()
                .copied()
                .filter(|tool| names.contains(*tool))
                .collect::<Vec<_>>();
            (!available_tools.is_empty()).then(|| {
                json!({
                    "id": group.id,
                    "description": group.description,
                    "tools": available_tools,
                })
            })
        })
        .collect()
}

fn available_workflows(names: &BTreeSet<String>) -> Vec<Value> {
    [
        workflow(
            "inspect_available_capabilities",
            "See what this server can actually do",
            "Start here when backend, version, or installed plugin surface is uncertain.",
            &[(
                DISCOVER_CAPABILITIES_TOOL,
                "Read the active version, backend, tools, capability groups, and generators.",
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
            "The level above one ceremony: roles, participants and several published ceremonies composed together. Validate before sealing, and seal the revision you read.",
            &[
                (DESIGN_AGENTIC_SYSTEM_TOOL, "Write the system down against the revision you read."),
                (VALIDATE_AGENTIC_SYSTEM_TOOL, "Resolve every pin and read every finding at once."),
                (PUBLISH_AGENTIC_SYSTEM_TOOL, "Seal that revision so a run can pin it."),
                (INSTANTIATE_AGENTIC_SYSTEM_TOOL, "Open a run, offering what your host can actually supply."),
                (ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL, "Start what is now ready; repeat as instances complete."),
            ],
        ),
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
    .into_iter()
    .filter(|workflow| workflow_tools(workflow).all(|tool| names.contains(tool)))
    .collect()
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

fn user_help(workflows: &[Value], names: &BTreeSet<String>) -> Value {
    let mut start_here = vec![
        "Describe the outcome, participants, stages, and any decision that must remain human.",
        "Ask to inspect capabilities when you are unsure which plugin build or backend is active.",
    ];
    let mut examples = vec![json!({
        "request": "What can this installed made do?",
        "first_tool": DISCOVER_CAPABILITIES_TOOL,
    })];
    if names.contains(DESIGN_CEREMONY_TOOL) {
        examples.push(json!({
            "request": "Design a review ceremony with two specialists and a final approval.",
            "first_tool": DESIGN_CEREMONY_TOOL,
        }));
    }
    if names.contains(DESIGN_AGENTIC_SYSTEM_TOOL) {
        start_here.push(
            "A system composes published ceremonies by pin; publish the ceremonies first, then compose them.",
        );
        examples.push(json!({
            "request": "Describe how our delivery team, its reviewer and its approver work together across three ceremonies.",
            "first_tool": DESIGN_AGENTIC_SYSTEM_TOOL,
            "note": "A capability no participant supplies makes a ceremony skip rather than run with a stand-in.",
        }));
    }
    if names.contains(GENERATE_CEREMONY_REPORT_TOOL) {
        start_here.push(
            "Ask for a report by ceremony id; made returns Markdown but does not choose a file path for you.",
        );
        examples.push(json!({
            "request": "Generate a report for ceremonies session-17 and session-18.",
            "first_tool": GENERATE_CEREMONY_REPORT_TOOL,
            "arguments": {"ceremony_ids": ["session-17", "session-18"]},
            "note": "The tool returns Markdown; your host decides where to save it.",
        }));
    }

    json!({
        "schema_version": SCHEMA_VERSION,
        "audience": "user",
        "summary": "made designs and coordinates explicit working sessions. The active backend determines which workflows are available.",
        "start_here": start_here,
        "authority": "A ceremony coordinates work and records declared decisions. It does not grant permission for external actions and it never turns an agent statement into human approval.",
        "workflows": workflows,
        "examples": examples,
    })
}

fn agent_help(workflows: &[Value], names: &BTreeSet<String>) -> Value {
    let mut preconditions = base_agent_preconditions();
    let mut authority_boundaries = base_agent_authority_boundaries();
    let mut execution_paths = Vec::new();
    if names.contains(crate::protocol::INSPECT_CEREMONY_RESUME_TOOL) {
        preconditions.push("After pause, inspect the resume preflight and all its pages. Paused admission is not host quiescence. Record authenticated handoff/quiescence/loss declarations with the original fence, worker incarnation and evidence before coordinated resume; evidence never authorizes takeover or extends clocks. Reconcile effects before replacing an expired claim.".to_owned());
    }

    if names.contains(crate::protocol::PLAN_CEREMONY_SUCCESSOR_TOOL) {
        preconditions.push("When a paused ceremony's definition turns out to be wrong, plan the successor before starting one: the plan reports the diff, the preflight, the evidence that would carry and the disposition every outstanding claim needs. Carrying evidence onto a stranded step is refused, a successor cannot be opened from a ceremony that is not paused, and a ceremony that has sealed a handoff can only be cancelled, never resumed.".to_owned());
    }

    if let Some(path) = server_owned_execution_path(names) {
        preconditions.push(format!(
            "For an ordinary handler step, use {RUN_CEREMONY_STEP_TOOL} only after verifying that the active host configured a real handler; the bundled default can be no-op. A declared spawn step instead runs the child orchestrator and never invokes its ordinary handler."
        ));
        execution_paths.push(path);
    }
    let delegated_host_sequence = delegated_host_sequence(names);
    if names.contains(crate::protocol::RENEW_CEREMONY_STEP_LEASE_TOOL) {
        preconditions.push("During long delegated work, call made_renew_ceremony_step_lease before effective expiry with the original owner/fence and a new renewal_id. Retry a lost response with the identical renewal_id and payload; it returns the earlier receipt, not new authority. Pause does not freeze deadlines. Renewal is not evidence of agent liveness.".to_owned());
    }
    if !delegated_host_sequence.is_empty() {
        preconditions.push(format!(
            "For host-owned work, require both {CLAIM_CEREMONY_STEP_TOOL} and {COMPLETE_CEREMONY_STEP_TOOL}; a claim alone is not evidence of execution."
        ));
        authority_boundaries.push(json!({
            "rule": "A delegated-host claim records a lease, not completion or evidence.",
            "forbidden_inference": "A successful claim means the external work happened."
        }));
        execution_paths.push(json!({
            "id": "delegated_host",
            "title": "Delegated-host execution",
            "when": "Use when the MCP host, its worker, or an external tool must perform the real stage work.",
            "sequence": delegated_host_sequence.clone(),
        }));
        if names.contains(LIST_CEREMONY_AGENTS_TOOL)
            && names.contains(GET_CEREMONY_AGENT_TOOL)
            && names.contains(REPORT_CEREMONY_AGENT_STATUS_TOOL)
        {
            preconditions.push(format!(
                "Use {LIST_CEREMONY_AGENTS_TOOL}/{GET_CEREMONY_AGENT_TOOL} for bounded authorized live visibility and {REPORT_CEREMONY_AGENT_STATUS_TOOL} only from an authenticated host claim; execution status and fresh/stale/unreachable liveness are separate."
            ));
            authority_boundaries.push(json!({
                "rule": "Host disappearance yields stale or unknown telemetry, never invented completion or failure; host discovery limitations are explicit.",
                "forbidden_inference": "A valid MADE lease proves an external agent is alive."
            }));
        }
    }
    let integrator_loop = integrator_loop_guidance(
        names,
        &mut preconditions,
        &mut authority_boundaries,
        &mut execution_paths,
    );
    if names.contains(INSPECT_EXECUTION_RECOVERY_TOOL) {
        preconditions.push(format!(
            "After an interrupted worker, inspect durable operation roots with {INSPECT_EXECUTION_RECOVERY_TOOL}. Use {COMPLETE_EXECUTION_RECEIPT_TOOL} only with the producer fence; use {ADOPT_EXECUTION_RECEIPT_TOOL} explicitly for a different current fence after reliable operation-id recovery."
        ));
    }
    if names.contains(GENERATE_CEREMONY_REPORT_TOOL) {
        authority_boundaries.push(json!({
            "rule": "Report generation is read-only.",
            "forbidden_inference": "persisted=false means a report file already exists."
        }));
    }

    json!({
        "schema_version": SCHEMA_VERSION,
        "audience": "agent",
        "summary": "Operational guidance for an agent or host driving made without inventing capability, evidence, or authority.",
        "preconditions": preconditions,
        "authority_boundaries": authority_boundaries,
        "execution_paths": execution_paths,
        "delegated_host_sequence": delegated_host_sequence,
        "integrator_loop_sequence": integrator_loop,
        "error_handling": [
            {
                "signal": "JSON-RPC error",
                "response": "Repair the protocol request before interpreting any domain outcome."
            },
            {
                "signal": "tool result isError=true",
                "response": "Surface the exact failure, do not advance the ceremony, and retry only after its cause changes."
            },
            {
                "signal": "tool is absent from discovery",
                "response": "Do not call it. Choose an advertised workflow or surface the backend limitation."
            },
            {
                "signal": "host context was lost",
                "response": "List and inspect existing instances before starting a successor."
            }
        ],
        "workflows": workflows,
    })
}

fn server_owned_execution_path(names: &BTreeSet<String>) -> Option<Value> {
    names.contains(RUN_CEREMONY_STEP_TOOL).then(|| {
        json!({
            "id": "server_owned_handler",
            "title": "Server-owned handler execution",
            "when": "Use for an ordinary handler step only when the active host configured a real CeremonyStepHandlerPort. A declared spawn step uses the child orchestrator instead and does not invoke this handler.",
            "default_warning": "The bundled embedded default may use NoopCeremonyStepHandler. A completed no-op proves state-machine wiring, not that search, scraping, modeling, rendering, or artifact creation occurred.",
            "sequence": [{
                "order": 1,
                "tool": RUN_CEREMONY_STEP_TOOL,
                "instruction": "Invoke the verified real server-owned handler and inspect its returned output/evidence before advancing."
            }]
        })
    })
}

fn render_help_markdown(help: &Value) -> String {
    let audience = help["audience"].as_str().unwrap_or("unknown");
    let summary = help["summary"].as_str().unwrap_or_default();
    let mut markdown = format!("# made help ({audience})\n\n{summary}\n");

    if audience == "user" {
        markdown.push_str("\n## Start here\n");
        render_string_list(&mut markdown, &help["start_here"]);
        if let Some(authority) = help["authority"].as_str() {
            let _ = write!(markdown, "\n## Authority boundary\n\n{authority}\n");
        }
        if let Some(examples) = help["examples"].as_array() {
            markdown.push_str("\n## Examples\n");
            for example in examples {
                let request = example["request"].as_str().unwrap_or_default();
                let tool = example["first_tool"].as_str().unwrap_or_default();
                let _ = write!(markdown, "\n- {request} Start with `{tool}`.");
            }
            markdown.push('\n');
        }
    } else if audience == "agent" {
        markdown.push_str("\n## Preconditions\n");
        render_string_list(&mut markdown, &help["preconditions"]);
        markdown.push_str("\n## Authority boundaries\n");
        for boundary in help["authority_boundaries"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let rule = boundary["rule"].as_str().unwrap_or_default();
            let forbidden = boundary["forbidden_inference"].as_str().unwrap_or_default();
            let _ = write!(markdown, "\n- {rule} Do not infer: {forbidden}");
        }
        markdown.push_str("\n\n## Execution paths\n");
        for path in help["execution_paths"].as_array().into_iter().flatten() {
            let title = path["title"].as_str().unwrap_or("Execution path");
            let when = path["when"].as_str().unwrap_or_default();
            let _ = write!(markdown, "\n### {title}\n\n{when}\n");
            if let Some(warning) = path["default_warning"].as_str() {
                let _ = write!(markdown, "\nWarning: {warning}\n");
            }
            render_sequence(&mut markdown, &path["sequence"]);
        }
        if help["delegated_host_sequence"]
            .as_array()
            .is_some_and(|steps| !steps.is_empty())
        {
            markdown.push_str("\n## Delegated-host sequence\n");
            render_sequence(&mut markdown, &help["delegated_host_sequence"]);
        }
        markdown.push_str("\n\n## Error handling\n");
        for error in help["error_handling"].as_array().into_iter().flatten() {
            let signal = error["signal"].as_str().unwrap_or_default();
            let response = error["response"].as_str().unwrap_or_default();
            let _ = write!(markdown, "\n- **{signal}:** {response}");
        }
        markdown.push('\n');
    }

    markdown.push_str("\n## Available workflows\n");
    for workflow in help["workflows"].as_array().into_iter().flatten() {
        let title = workflow["title"].as_str().unwrap_or("Workflow");
        let details = workflow["summary"].as_str().unwrap_or_default();
        let _ = write!(markdown, "\n### {title}\n\n{details}\n");
        for step in workflow["steps"].as_array().into_iter().flatten() {
            let order = step["order"].as_u64().unwrap_or_default();
            let tool = step["tool"].as_str().unwrap_or_default();
            let purpose = step["purpose"].as_str().unwrap_or_default();
            let _ = write!(markdown, "\n{order}. `{tool}` — {purpose}");
        }
        markdown.push('\n');
    }
    markdown
}

fn render_sequence(markdown: &mut String, sequence: &Value) {
    for step in sequence.as_array().into_iter().flatten() {
        let order = step["order"].as_u64().unwrap_or_default();
        let instruction = step["instruction"].as_str().unwrap_or_default();
        if let Some(tool) = step["tool"].as_str() {
            let _ = write!(markdown, "\n{order}. `{tool}` — {instruction}");
        } else {
            let _ = write!(markdown, "\n{order}. {instruction}");
        }
    }
    markdown.push('\n');
}

fn render_string_list(markdown: &mut String, values: &Value) {
    for value in values.as_array().into_iter().flatten() {
        if let Some(value) = value.as_str() {
            let _ = write!(markdown, "\n- {value}");
        }
    }
    markdown.push('\n');
}

#[derive(Clone, Copy)]
enum HelpAudience {
    User,
    Agent,
}

fn parse_audience(arguments: &Value) -> Result<HelpAudience, String> {
    let object = arguments
        .as_object()
        .ok_or_else(|| "made_get_help arguments must be an object".to_owned())?;
    reject_unknown_fields(object, &["audience"])?;
    match object.get("audience").and_then(Value::as_str) {
        Some("user") => Ok(HelpAudience::User),
        Some("agent") => Ok(HelpAudience::Agent),
        Some(other) => Err(format!(
            "made_get_help audience must be `user` or `agent`, got `{other}`"
        )),
        None => Err("made_get_help requires string field `audience`".to_owned()),
    }
}

fn reject_unknown_fields(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    let unknown = object
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "made_get_help received unknown fields: {}",
            unknown.join(", ")
        ))
    }
}

fn catalog_names(catalog: &[Value]) -> BTreeSet<String> {
    catalog
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .map(str::to_owned)
        .collect()
}

fn workflow_tools(workflow: &Value) -> impl Iterator<Item = &str> {
    workflow["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|step| step["tool"].as_str())
}

#[cfg(test)]
mod tests {
    use super::integrator_loop_sequence::HALTING_STATES;
    use super::*;
    use crate::protocol::{is_grpc_tool, is_server_tool};
    use crate::protocol::{AWAIT_INTEGRATOR_ATTENTION_TOOL, BIND_CEREMONY_INTEGRATOR_TOOL};

    #[test]
    fn discovery_is_derived_from_the_active_catalog_and_marks_report_generator() {
        let result = discovery_result(
            McpServerIdentity::new("test-mcp", "9.8.7"),
            "embedded",
            "disabled",
            HostActivationAdapterKind::None,
            &json!({}),
            |_| true,
        )
        .unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(result["tool_count"].as_u64().unwrap() as usize, tools.len());
        assert_eq!(result["server"]["version"], "9.8.7");
        let report = tools
            .iter()
            .find(|tool| tool["name"] == GENERATE_CEREMONY_REPORT_TOOL)
            .unwrap();
        assert_eq!(report["report_generator"], true);
        assert_eq!(
            result["artifact_generators"][0]["response_field"],
            "structuredContent.report_markdown"
        );
        assert_eq!(result["design_patterns"][0]["id"], "roundtable_fixed_order");
        assert!(result["design_patterns"][0]["definition_fragment_yaml"]
            .as_str()
            .is_some_and(|yaml| yaml.contains("name: \"roundtable_fixed_order\"")));
    }

    /// Discovery advertises what the active backend can run, and
    /// nothing else.
    ///
    /// The gRPC backend no longer filters anything out: every ceremony
    /// tool has an RPC behind it since parity slice F3c, the report
    /// included, so that is asserted rather than assumed. The
    /// filtering itself is shown with a backend that serves nothing
    /// but the server's own two tools.
    #[test]
    fn discovery_hides_backend_tools_the_backend_cannot_execute() {
        let names = |supports: fn(&str) -> bool| {
            discovery_result(
                McpServerIdentity::new("test-mcp", "1.0.0"),
                "fixture",
                "disabled",
                HostActivationAdapterKind::None,
                &json!({}),
                supports,
            )
            .unwrap()
        };

        let over_the_wire = names(is_grpc_tool);
        let advertised = over_the_wire["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert!(advertised.contains(DISCOVER_CAPABILITIES_TOOL));
        assert!(advertised.contains(GET_HELP_TOOL));
        assert!(advertised.contains(GENERATE_CEREMONY_REPORT_TOOL));
        assert!(advertised.contains(READ_CEREMONY_EVENTS_TOOL));
        assert!(advertised.contains(GET_CEREMONY_TRANSCRIPT_TOOL));
        assert!(!over_the_wire["artifact_generators"]
            .as_array()
            .unwrap()
            .is_empty());

        let bare = names(is_server_tool);
        let advertised = bare["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            advertised,
            BTreeSet::from([DISCOVER_CAPABILITIES_TOOL, GET_HELP_TOOL])
        );
        assert!(bare["artifact_generators"].as_array().unwrap().is_empty());
        assert!(bare["design_patterns"].as_array().unwrap().is_empty());
    }

    #[test]
    fn capability_groups_cover_every_advertised_tool() {
        let result = discovery_result(
            McpServerIdentity::new("test-mcp", "1.0.0"),
            "all",
            "disabled",
            HostActivationAdapterKind::None,
            &json!({}),
            supports_all,
        )
        .unwrap();
        let advertised = result["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>();
        let grouped = result["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|capability| capability["tools"].as_array().unwrap())
            .map(|tool| tool.as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>();

        assert_eq!(grouped, advertised);
    }

    #[test]
    fn every_help_tool_reference_is_advertised_for_that_backend() {
        for (backend, supports) in [
            ("all", supports_all as fn(&str) -> bool),
            ("grpc", is_grpc_tool as fn(&str) -> bool),
        ] {
            let catalog = available_tool_catalog(supports);
            let advertised = catalog_names(&catalog);
            for audience in ["user", "agent"] {
                let help = help_result(&json!({"audience": audience}), supports).unwrap();
                assert_help_tool_references_are_advertised(&help, &advertised, backend);
                if audience == "agent" {
                    let markdown = help["help_markdown"].as_str().unwrap();
                    assert!(markdown.contains("## Preconditions"));
                    assert!(markdown.contains("## Authority boundaries"));
                    assert!(markdown.contains("## Error handling"));
                    // On both backends now: the delegated-host
                    // protocol reached the contract in parity slice
                    // F3a, so a host that delegates step execution
                    // gets the same instructions against a cluster.
                    assert!(
                        markdown.contains("## Delegated-host sequence"),
                        "{backend} agent help drops the delegated-host sequence"
                    );
                    assert!(markdown.contains("Integrator is a business responsibility"));
                    assert!(markdown.contains("prove host-agent activity"));
                    assert!(markdown.contains("requested and actual model/effort"));
                    assert!(markdown.contains("Codex/Claude agent id is a MADE role id"));
                }
            }
        }
    }

    /// Help offers what the backend can actually run, and nothing
    /// else.
    ///
    /// This test used to assert the opposite about reporting: it had
    /// no RPC behind it, so recommending it to a client pointed at a
    /// cluster would have sent that client to a tool it could not
    /// call. Parity slice F3c gave it one, along with the two reads,
    /// so a host against a cluster is now told about all three — and
    /// about the delegated-host protocol, which got its RPCs in F3a.
    #[test]
    fn grpc_help_offers_reporting_history_and_delegated_execution() {
        let user = help_result(&json!({"audience": "user"}), is_grpc_tool).unwrap();
        let user_text = serde_json::to_string(&user).unwrap().to_ascii_lowercase();
        assert!(
            user_text.contains("report"),
            "grpc user help drops the reporting it can now serve: {user_text}"
        );

        let agent = help_result(&json!({"audience": "agent"}), is_grpc_tool).unwrap();
        let sequence = agent["delegated_host_sequence"].as_array().unwrap();
        assert!(
            !sequence.is_empty(),
            "grpc agent help drops the delegated-host sequence it can serve"
        );
        let agent_text = serde_json::to_string(&agent).unwrap();
        assert!(agent_text.contains(CLAIM_CEREMONY_STEP_TOOL));
        assert!(agent_text.contains(COMPLETE_CEREMONY_STEP_TOOL));
        assert!(agent_text
            .to_ascii_lowercase()
            .contains("report generation"));

        // Reading what a session recorded is a workflow on this
        // backend now, not only in process.
        let workflow_ids = agent["workflows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|workflow| workflow["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert!(workflow_ids.contains("inspect_ceremony_history"), "{agent}");
        assert!(workflow_ids.contains("generate_report"), "{agent}");
    }

    #[test]
    fn agent_help_separates_real_server_handlers_from_delegated_host_work() {
        let help = help_result(&json!({"audience": "agent"}), supports_all).unwrap();
        let paths = help["execution_paths"].as_array().unwrap();
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0]["id"], "server_owned_handler");
        assert!(paths[0]["default_warning"]
            .as_str()
            .unwrap()
            .contains("NoopCeremonyStepHandler"));
        assert_eq!(paths[1]["id"], "delegated_host");
        assert_eq!(paths[2]["id"], "integrator_loop");

        let sequence = help["delegated_host_sequence"].as_array().unwrap();
        assert_eq!(sequence[0]["tool"], CLAIM_CEREMONY_STEP_TOOL);
        assert_eq!(sequence[1]["host_action"], true);
        assert_eq!(sequence[2]["tool"], COMPLETE_CEREMONY_STEP_TOOL);
        assert!(sequence[2]["instruction"]
            .as_str()
            .unwrap()
            .contains("evidence/artifact references"));
    }

    #[test]
    fn agent_help_tells_an_integrator_to_revalidate_and_to_stop() {
        let help = help_result(&json!({"audience": "agent"}), supports_all).unwrap();
        let sequence = help["integrator_loop_sequence"].as_array().unwrap();
        assert_eq!(sequence[0]["tool"], BIND_CEREMONY_INTEGRATOR_TOOL);
        // Reading the instance again is a step of its own, between
        // being handed news and acting on it.
        assert_eq!(sequence[2]["tool"], GET_CEREMONY_INSTANCE_TOOL);
        assert_eq!(sequence[4]["host_action"], true);
        let last = sequence.last().unwrap()["instruction"].as_str().unwrap();
        for halting in HALTING_STATES {
            assert!(last.contains(halting), "{last}");
        }
        assert!(last.contains("delivery_id"), "{last}");
    }

    #[test]
    fn discovery_says_which_activation_adapter_this_deployment_has() {
        let result = discovery_result(
            McpServerIdentity::new("made-mcp", "9.9.9"),
            "embedded",
            "disabled",
            HostActivationAdapterKind::None,
            &Value::Null,
            supports_all,
        )
        .unwrap();
        assert_eq!(result["host_activation"]["adapter"], "none");
        assert_eq!(
            result["host_activation"]["bounded_follow"],
            AWAIT_INTEGRATOR_ATTENTION_TOOL
        );
        let woken = discovery_result(
            McpServerIdentity::new("made-mcp", "9.9.9"),
            "embedded",
            "disabled",
            HostActivationAdapterKind::Command,
            &Value::Null,
            supports_all,
        )
        .unwrap();
        assert_eq!(woken["host_activation"]["adapter"], "command");
    }

    #[test]
    fn help_rejects_missing_invalid_and_extra_fields() {
        for arguments in [
            json!({}),
            json!({"audience": "operator"}),
            json!({"audience": "user", "extra": true}),
            Value::Null,
        ] {
            assert!(help_result(&arguments, |_| true).is_err());
        }
    }

    #[test]
    fn discovery_rejects_non_empty_arguments() {
        assert!(discovery_result(
            McpServerIdentity::new("test-mcp", "1.0.0"),
            "fixture",
            "disabled",
            HostActivationAdapterKind::None,
            &json!({"unexpected": true}),
            is_grpc_tool,
        )
        .is_err());
    }

    #[test]
    fn server_tool_catalog_membership_is_complete() {
        assert!(is_server_tool(DISCOVER_CAPABILITIES_TOOL));
        assert!(is_server_tool(GET_HELP_TOOL));
        assert!(!is_server_tool(GENERATE_CEREMONY_REPORT_TOOL));
    }

    fn assert_help_tool_references_are_advertised(
        value: &Value,
        advertised: &BTreeSet<String>,
        backend: &str,
    ) {
        match value {
            Value::Object(object) => {
                if let Some(tool) = object.get("tool").and_then(Value::as_str) {
                    assert!(
                        advertised.contains(tool),
                        "{backend} help references unadvertised tool {tool}"
                    );
                }
                if let Some(tool) = object.get("first_tool").and_then(Value::as_str) {
                    assert!(
                        advertised.contains(tool),
                        "{backend} help references unadvertised first tool {tool}"
                    );
                }
                for child in object.values() {
                    assert_help_tool_references_are_advertised(child, advertised, backend);
                }
            }
            Value::Array(values) => {
                for child in values {
                    assert_help_tool_references_are_advertised(child, advertised, backend);
                }
            }
            Value::String(text) => {
                for tool in tool_names_in_text(text) {
                    assert!(
                        advertised.contains(tool),
                        "{backend} help text references unadvertised tool {tool}: {text}"
                    );
                }
            }
            _ => {}
        }
    }

    fn tool_names_in_text(text: &str) -> Vec<&str> {
        text.match_indices("made_")
            .map(|(start, _)| {
                let suffix = &text[start..];
                let end = suffix
                    .char_indices()
                    .find_map(|(index, character)| {
                        (!(character.is_ascii_lowercase()
                            || character.is_ascii_digit()
                            || character == '_'))
                            .then_some(index)
                    })
                    .unwrap_or(suffix.len());
                &suffix[..end]
            })
            .collect()
    }

    fn supports_all(_: &str) -> bool {
        true
    }
}
