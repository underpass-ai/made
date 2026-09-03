use super::*;
use serde_json::{json, Value};

#[test]
fn initialize_advertises_protocol_version_and_metadata() {
    let r = initialize_result("host-mcp", "1.2.3", "grpc", "server");
    assert_eq!(r["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(r["serverInfo"]["name"], "host-mcp");
    assert_eq!(r["serverInfo"]["version"], "1.2.3");
    assert_eq!(r["metadata"]["backend"], "grpc");
    assert_eq!(r["metadata"]["grpc_tls"], "server");
}

#[test]
fn tools_catalog_is_derived_one_for_one_from_grpc_service() {
    let catalog_names = grpc_catalog_tool_names();
    let proto_tool_names: Vec<String> = proto_rpc_names()
        .into_iter()
        .map(rpc_name_to_tool_name)
        .collect();
    let supported_tool_names = GRPC_TOOL_NAMES.map(str::to_owned).to_vec();

    assert_eq!(catalog_names, supported_tool_names);
    assert_eq!(
        catalog_names, proto_tool_names,
        "every underpass.made.v1 gRPC RPC must have exactly one MCP tool"
    );
}

#[test]
fn grpc_dispatch_and_fixture_cover_every_catalog_tool() {
    let grpc_dispatch_source = include_str!("../grpc/tools.rs");
    let fixture_source = include_str!("../fixture.rs");

    for tool in grpc_catalog_tool_names() {
        let dispatch_arm = format!("\"{tool}\" =>");
        assert!(
            grpc_dispatch_source.contains(&dispatch_arm),
            "live gRPC backend is missing a dispatch arm for {tool}"
        );
        assert!(
            fixture_source.contains(&dispatch_arm),
            "fixture backend is missing a canned response for {tool}"
        );
    }
}

#[test]
fn incremental_ceremony_tools_are_unique_catalog_extensions() {
    let all_names = catalog_tool_names();
    let unique_names = all_names.iter().collect::<std::collections::BTreeSet<_>>();

    assert_eq!(all_names.len(), 41);
    assert_eq!(unique_names.len(), all_names.len());
    assert!(all_names.contains(&VALIDATE_CEREMONY_DRAFT_TOOL.to_owned()));
    assert!(all_names.contains(&PUBLISH_CEREMONY_DEFINITION_TOOL.to_owned()));
    assert!(all_names.contains(&START_PUBLISHED_CEREMONY_TOOL.to_owned()));
    assert!(all_names.contains(&EXPLAIN_CEREMONY_DRAFT_TOOL.to_owned()));
    assert!(all_names.contains(&START_CEREMONY_TOOL.to_owned()));
    assert!(all_names.contains(&APPROVE_CEREMONY_GUARD_TOOL.to_owned()));
    assert!(all_names.contains(&DEFER_CEREMONY_GUARD_TOOL.to_owned()));
    assert!(all_names.contains(&GET_CEREMONY_INSTANCE_TOOL.to_owned()));
    assert!(all_names.contains(&LIST_CEREMONY_INSTANCES_TOOL.to_owned()));
    assert!(all_names.contains(&REQUEST_CEREMONY_INTERVENTION_TOOL.to_owned()));
    assert!(all_names.contains(&RESPOND_TO_CEREMONY_INTERVENTION_TOOL.to_owned()));
    assert!(all_names.contains(&CLOSE_CEREMONY_INTERVENTION_TOOL.to_owned()));
    assert!(all_names.contains(&COLLECT_CEREMONY_EVIDENCE_TOOL.to_owned()));
    assert!(all_names.contains(&DESIGN_CEREMONY_TOOL.to_owned()));
    assert!(all_names.contains(&CLAIM_CEREMONY_STEP_TOOL.to_owned()));
    assert!(all_names.contains(&COMPLETE_CEREMONY_STEP_TOOL.to_owned()));
    assert!(all_names.contains(&GENERATE_CEREMONY_REPORT_TOOL.to_owned()));
    assert!(all_names.contains(&DISCOVER_CAPABILITIES_TOOL.to_owned()));
    assert!(all_names.contains(&GET_HELP_TOOL.to_owned()));
}

#[test]
fn task_schema_includes_metadata_and_external_context() {
    let s = task_schema();
    let props = &s["properties"];
    assert!(props.get("task_id").is_some());
    assert!(props.get("description").is_some());
    assert!(props.get("specialty").is_some());
    assert!(props.get("constraints").is_some());
    assert!(props.get("attributes").is_some());
    assert!(props.get("external_context").is_some());
    assert!(props.get("metadata").is_some());
}

#[test]
fn output_contract_format_enum_pins_implemented_modes() {
    let s = output_contract_schema();
    let formats = s["properties"]["format"]["enum"].as_array().unwrap();
    assert_eq!(formats.len(), 1);
    assert_eq!(formats[0], "json_object");
}

#[test]
fn tool_results_carry_both_text_and_structured() {
    let success = tool_success_result(json!({"answer": "yes"}));
    assert_eq!(success["isError"], false);
    assert_eq!(success["structuredContent"]["answer"], "yes");
    assert!(success["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("yes"));

    let error = tool_error_result("nope");
    assert_eq!(error["isError"], true);
    assert_eq!(error["content"][0]["text"], "nope");
}

#[test]
fn jsonrpc_helpers_wrap_results_and_errors() {
    let r = serde_json::from_str::<Value>(&jsonrpc_result(json!(1), json!({"x": 2}))).unwrap();
    assert_eq!(r["jsonrpc"], "2.0");
    assert_eq!(r["id"], 1);
    assert_eq!(r["result"]["x"], 2);

    let e = serde_json::from_str::<Value>(&jsonrpc_error(json!(2), -32601, "no")).unwrap();
    assert_eq!(e["error"]["code"], -32601);
    assert_eq!(e["error"]["message"], "no");
}

fn catalog_tool_names() -> Vec<String> {
    let tools = tools_list_result(|_| true);
    tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect()
}

fn grpc_catalog_tool_names() -> Vec<String> {
    grpc_tool_catalog()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_owned())
        .collect()
}

fn proto_rpc_names() -> Vec<&'static str> {
    const MADE_PROTO: &str =
        include_str!("../../../made-mcp-proto/proto/underpass/made/v1/made.proto");

    MADE_PROTO
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let rest = trimmed.strip_prefix("rpc ")?;
            rest.split_once('(').map(|(rpc, _)| rpc.trim())
        })
        .collect()
}

fn rpc_name_to_tool_name(rpc: &str) -> String {
    let mut snake = String::new();
    for (idx, ch) in rpc.chars().enumerate() {
        if ch.is_uppercase() {
            if idx > 0 {
                snake.push('_');
            }
            snake.extend(ch.to_lowercase());
        } else {
            snake.push(ch);
        }
    }
    format!("made_{snake}")
}
