//! RPC responses back to the JSON a caller reads.
//!
//! Where a response carries the projection the engine rendered, this
//! hands it back untouched. Re-deriving it here would be a second
//! renderer, and the whole point of the shared one is that a client
//! switching backends reads the same words about the same design.

use made_mcp_proto::v1 as pb;
use prost_types::Struct as PbStruct;
use serde_json::{json, Value};

use crate::grpc::proto_to_json::pb_struct_to_json;
use crate::protocol::ToolError;

pub(super) fn system(state: Option<pb::AgenticSystemState>) -> Result<Value, ToolError> {
    rendered(state.and_then(|state| state.json), "agentic system")
}

pub(super) fn page(response: pb::ListAgenticSystemsResponse) -> Result<Value, ToolError> {
    let systems = response
        .systems
        .into_iter()
        .map(|state| rendered(state.json, "agentic system"))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "systems": systems,
        "next_cursor": optional(response.next_cursor),
    }))
}

pub(super) fn execution(
    state: Option<pb::AgenticSystemExecutionState>,
) -> Result<Value, ToolError> {
    rendered(state.and_then(|state| state.json), "agentic system run")
}

pub(super) fn validation(response: pb::ValidateAgenticSystemResponse) -> Value {
    json!({
        "system_id": response.system_id,
        "revision": response.revision,
        "publishable": response.publishable,
        "error_count": response.error_count,
        "warning_count": response.warning_count,
        "findings": response
            .findings
            .into_iter()
            .map(|finding| json!({
                "severity": finding.severity,
                "locus": finding.locus.map_or(Value::Null, struct_to_json),
                "message": finding.message,
            }))
            .collect::<Vec<_>>(),
        "resolved_pins": response
            .resolved_pins
            .into_iter()
            .map(|pin| json!({
                "ceremony": pin.ceremony,
                "name": pin.name,
                "version": pin.version,
                "digest": pin.digest,
            }))
            .collect::<Vec<_>>(),
    })
}

pub(super) fn publication(response: pb::PublishAgenticSystemResponse) -> Value {
    json!({
        "outcome": response.outcome,
        "system_id": response.system_id,
        "sealed_revision": response.sealed_revision,
        "head_revision": response.head_revision,
        "digest": optional(response.digest),
    })
}

pub(super) fn diagram(response: pb::RenderAgenticSystemDiagramResponse) -> Value {
    json!({
        "mermaid": response.mermaid,
        "text_equivalent": response.text_equivalent,
    })
}

/// The engine's own projection, or an honest refusal when the
/// response arrived without one.
fn rendered(payload: Option<PbStruct>, what: &str) -> Result<Value, ToolError> {
    payload
        .map(struct_to_json)
        .ok_or_else(|| ToolError::refused(format!("made returned no {what}")))
}

/// An empty string is proto3's "not set", and JSON says that with
/// null rather than with an empty string that reads like a value.
fn optional(value: String) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value)
    }
}

fn struct_to_json(value: PbStruct) -> Value {
    Value::Object(pb_struct_to_json(value))
}
