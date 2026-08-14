//! Server-owned discovery and audience guidance.
//!
//! These responses are projected from the same backend-filtered catalog used
//! by MCP `tools/list`. They therefore describe the server that answered the
//! call, rather than a separately maintained promise about some other build.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde_json::{json, Map, Value};

use crate::mcp_server_identity::McpServerIdentity;
use crate::protocol::{
    available_tool_catalog, APPLY_CEREMONY_TRANSITION_TOOL, APPROVE_CEREMONY_GUARD_TOOL,
    ASSERT_CEREMONY_REASON_TOOL, BIND_CEREMONY_PARTICIPANTS_TOOL, CLOSE_CEREMONY_INTERVENTION_TOOL,
    COLLECT_CEREMONY_EVIDENCE_TOOL, DEFER_CEREMONY_GUARD_TOOL, DESIGN_CEREMONY_TOOL,
    DIFF_CEREMONY_DEFINITIONS_TOOL, DISCOVER_CAPABILITIES_TOOL, EXPLAIN_CEREMONY_DRAFT_TOOL,
    GENERATE_CEREMONY_REPORT_TOOL, GET_CEREMONY_INSTANCE_TOOL, GET_HELP_TOOL,
    LIST_CEREMONY_INSTANCES_TOOL, PUBLISH_CEREMONY_DEFINITION_TOOL,
    REQUEST_CEREMONY_INTERVENTION_TOOL, RESPOND_TO_CEREMONY_INTERVENTION_TOOL,
    RUN_CEREMONY_STEP_TOOL, RUN_CEREMONY_TOOL, START_CEREMONY_TOOL, START_PUBLISHED_CEREMONY_TOOL,
    VALIDATE_CEREMONY_DRAFT_TOOL,
};

const SCHEMA_VERSION: &str = "1.0";

struct CapabilityGroup {
    id: &'static str,
    description: &'static str,
    tools: &'static [&'static str],
}

const CAPABILITY_GROUPS: &[CapabilityGroup] = &[
    CapabilityGroup {
        id: "self_description",
        description: "Inspect the active server and obtain audience-specific help.",
        tools: &[DISCOVER_CAPABILITIES_TOOL, GET_HELP_TOOL],
    },
    CapabilityGroup {
        id: "council_deliberation",
        description: "Run, inspect, trigger, and optionally execute council deliberations.",
        tools: &[
            "choreo_deliberate",
            "choreo_stream_deliberation",
            "choreo_get_deliberation_result",
            "choreo_orchestrate",
            "choreo_process_trigger_event",
            "choreo_run_council_decision",
        ],
    },
    CapabilityGroup {
        id: "council_configuration",
        description: "Manage councils, agents, and output contracts.",
        tools: &[
            "choreo_create_council",
            "choreo_list_councils",
            "choreo_delete_council",
            "choreo_register_agent",
            "choreo_unregister_agent",
            "choreo_register_contract",
            "choreo_list_contracts",
            "choreo_delete_contract",
        ],
    },
    CapabilityGroup {
        id: "ceremony_design",
        description: "Design, validate, explain, compare, and publish ceremony definitions.",
        tools: &[
            DESIGN_CEREMONY_TOOL,
            VALIDATE_CEREMONY_DRAFT_TOOL,
            EXPLAIN_CEREMONY_DRAFT_TOOL,
            PUBLISH_CEREMONY_DEFINITION_TOOL,
            DIFF_CEREMONY_DEFINITIONS_TOOL,
        ],
    },
    CapabilityGroup {
        id: "ceremony_execution",
        description: "Run a ceremony in one shot or drive a persistent instance step by step.",
        tools: &[
            RUN_CEREMONY_TOOL,
            START_CEREMONY_TOOL,
            START_PUBLISHED_CEREMONY_TOOL,
            RUN_CEREMONY_STEP_TOOL,
            APPLY_CEREMONY_TRANSITION_TOOL,
        ],
    },
    CapabilityGroup {
        id: "ceremony_recovery",
        description: "Rediscover and inspect instances owned by the active backend.",
        tools: &[LIST_CEREMONY_INSTANCES_TOOL, GET_CEREMONY_INSTANCE_TOOL],
    },
    CapabilityGroup {
        id: "human_authorization",
        description: "Record explicit guard decisions without inferring human authority.",
        tools: &[APPROVE_CEREMONY_GUARD_TOOL, DEFER_CEREMONY_GUARD_TOOL],
    },
    CapabilityGroup {
        id: "ceremony_participation",
        description:
            "Seat participants, coordinate interventions, attach evidence, and record reasons.",
        tools: &[
            BIND_CEREMONY_PARTICIPANTS_TOOL,
            REQUEST_CEREMONY_INTERVENTION_TOOL,
            RESPOND_TO_CEREMONY_INTERVENTION_TOOL,
            CLOSE_CEREMONY_INTERVENTION_TOOL,
            COLLECT_CEREMONY_EVIDENCE_TOOL,
            ASSERT_CEREMONY_REASON_TOOL,
        ],
    },
    CapabilityGroup {
        id: "service_observability",
        description: "Inspect service health and statistics.",
        tools: &["choreo_get_status", "choreo_get_metrics"],
    },
    CapabilityGroup {
        id: "ceremony_reporting",
        description: "Project persisted ceremony state and journals into Markdown.",
        tools: &[GENERATE_CEREMONY_REPORT_TOOL],
    },
];

/// Describe exactly the tools executable through the active server.
pub(crate) fn discovery_result(
    identity: McpServerIdentity,
    backend: &str,
    grpc_tls: &str,
    arguments: &Value,
    supports: impl Fn(&str) -> bool,
) -> Result<Value, String> {
    match arguments {
        Value::Null => {}
        Value::Object(object) if object.is_empty() => {}
        Value::Object(object) => {
            return Err(format!(
                "choreo_discover_capabilities accepts no fields, got: {}",
                object.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        _ => {
            return Err("choreo_discover_capabilities arguments must be an object".to_owned());
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
        "artifact_generators": artifact_generators,
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
    let mut examples = vec![json!({
        "request": "What can this installed Choreographer do?",
        "first_tool": DISCOVER_CAPABILITIES_TOOL,
    })];
    if names.contains(DESIGN_CEREMONY_TOOL) {
        examples.push(json!({
            "request": "Design a review ceremony with two specialists and a final approval.",
            "first_tool": DESIGN_CEREMONY_TOOL,
        }));
    }
    if names.contains(GENERATE_CEREMONY_REPORT_TOOL) {
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
        "summary": "Choreographer designs and coordinates explicit working sessions. The active backend determines which workflows are available.",
        "start_here": [
            "Describe the outcome, participants, stages, and any decision that must remain human.",
            "Ask to inspect capabilities when you are unsure which plugin build or backend is active.",
            "Ask for a report by ceremony id; Choreographer returns Markdown but does not choose a file path for you."
        ],
        "authority": "A ceremony coordinates work and records declared decisions. It does not grant permission for external actions and it never turns an agent statement into human approval.",
        "workflows": workflows,
        "examples": examples,
    })
}

fn agent_help(workflows: &[Value], names: &BTreeSet<String>) -> Value {
    let report_step = names.contains(GENERATE_CEREMONY_REPORT_TOOL).then(|| {
        json!({
            "order": 6,
            "tool": GENERATE_CEREMONY_REPORT_TOOL,
            "instruction": "Generate the final Markdown projection, verify isError=false, then let the host persist report_markdown at the user-approved destination."
        })
    });
    let mut delegated_host_sequence = vec![
        json!({
            "order": 1,
            "tool": DISCOVER_CAPABILITIES_TOOL,
            "instruction": "Discover the current server before planning; never assume another installation's tool surface."
        }),
        json!({
            "order": 2,
            "instruction": "Choose only a workflow whose complete tool sequence is advertised."
        }),
        json!({
            "order": 3,
            "instruction": "When a ceremony step requires specialist work, the host delegates through its own authorized agent/tool mechanism; Choreographer coordinates and records the result but does not create that authority."
        }),
        json!({
            "order": 4,
            "tool": RUN_CEREMONY_STEP_TOOL,
            "instruction": "Submit the exact declared step after real work is complete, then re-read the returned state."
        }),
        json!({
            "order": 5,
            "tool": APPLY_CEREMONY_TRANSITION_TOOL,
            "instruction": "Apply only a transition reported as enabled; pause for any unresolved human guard or intervention."
        }),
    ];
    delegated_host_sequence.retain(|step| {
        step.get("tool")
            .and_then(Value::as_str)
            .is_none_or(|tool| names.contains(tool))
    });
    if let Some(report_step) = report_step {
        delegated_host_sequence.push(report_step);
    }

    json!({
        "schema_version": SCHEMA_VERSION,
        "audience": "agent",
        "summary": "Operational guidance for an agent or host driving Choreographer without inventing capability, evidence, or authority.",
        "preconditions": [
            format!("Call {DISCOVER_CAPABILITIES_TOOL} and plan against its returned tools, backend, and version."),
            "Preserve stable ceremony ids and actor identity across calls.",
            "Have the exact definition, required context, and host permissions before starting.",
            "Treat isError=true, completed=false, and missing evidence as explicit non-success."
        ],
        "authority_boundaries": [
            {
                "rule": "Human guard approval requires a person's explicit current authorization.",
                "forbidden_inference": "Silence, prior approval, an agent recommendation, or operational convenience."
            },
            {
                "rule": "Interventions coordinate requests but grant no external mutation authority.",
                "forbidden_inference": "An action request is permission to alter another system."
            },
            {
                "rule": "Evidence must come from an actual authorized source and remain attributable.",
                "forbidden_inference": "An empty, inaccessible, or imagined source is evidence."
            },
            {
                "rule": "Report generation is read-only.",
                "forbidden_inference": "persisted=false means a report file already exists."
            }
        ],
        "delegated_host_sequence": delegated_host_sequence,
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
                "response": "Do not call it. Choose an advertised workflow or report the backend limitation."
            },
            {
                "signal": "host context was lost",
                "response": "List and inspect existing instances before starting a successor."
            }
        ],
        "workflows": workflows,
    })
}

fn render_help_markdown(help: &Value) -> String {
    let audience = help["audience"].as_str().unwrap_or("unknown");
    let summary = help["summary"].as_str().unwrap_or_default();
    let mut markdown = format!("# Choreographer help ({audience})\n\n{summary}\n");

    if audience == "user" {
        markdown.push_str("\n## Start here\n");
        render_string_list(&mut markdown, &help["start_here"]);
        if let Some(authority) = help["authority"].as_str() {
            let _ = write!(markdown, "\n## Authority boundary\n\n{authority}\n");
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
        markdown.push_str("\n\n## Delegated-host sequence\n");
        for step in help["delegated_host_sequence"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let order = step["order"].as_u64().unwrap_or_default();
            let instruction = step["instruction"].as_str().unwrap_or_default();
            if let Some(tool) = step["tool"].as_str() {
                let _ = write!(markdown, "\n{order}. `{tool}` — {instruction}");
            } else {
                let _ = write!(markdown, "\n{order}. {instruction}");
            }
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
        .ok_or_else(|| "choreo_get_help arguments must be an object".to_owned())?;
    reject_unknown_fields(object, &["audience"])?;
    match object.get("audience").and_then(Value::as_str) {
        Some("user") => Ok(HelpAudience::User),
        Some("agent") => Ok(HelpAudience::Agent),
        Some(other) => Err(format!(
            "choreo_get_help audience must be `user` or `agent`, got `{other}`"
        )),
        None => Err("choreo_get_help requires string field `audience`".to_owned()),
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
            "choreo_get_help received unknown fields: {}",
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
    use super::*;
    use crate::protocol::{is_grpc_tool, is_server_tool};

    #[test]
    fn discovery_is_derived_from_the_active_catalog_and_marks_report_generator() {
        let result = discovery_result(
            McpServerIdentity::new("test-mcp", "9.8.7"),
            "embedded",
            "disabled",
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
    }

    #[test]
    fn discovery_hides_backend_tools_the_backend_cannot_execute() {
        let result = discovery_result(
            McpServerIdentity::new("test-mcp", "1.0.0"),
            "fixture",
            "disabled",
            &json!({}),
            is_grpc_tool,
        )
        .unwrap();
        let names = result["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert!(names.contains(DISCOVER_CAPABILITIES_TOOL));
        assert!(names.contains(GET_HELP_TOOL));
        assert!(!names.contains(GENERATE_CEREMONY_REPORT_TOOL));
        assert!(result["artifact_generators"].as_array().unwrap().is_empty());
    }

    #[test]
    fn capability_groups_cover_every_advertised_tool() {
        let result = discovery_result(
            McpServerIdentity::new("test-mcp", "1.0.0"),
            "all",
            "disabled",
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
                    assert!(markdown.contains("## Delegated-host sequence"));
                    assert!(markdown.contains("## Error handling"));
                }
            }
        }
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
            _ => {}
        }
    }

    fn supports_all(_: &str) -> bool {
        true
    }
}
