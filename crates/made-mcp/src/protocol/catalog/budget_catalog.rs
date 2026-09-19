use serde_json::Value;

use super::super::budget_schemas::{budget_report_schema, pending_budget_reservations_schema};
use super::super::tool_names::{
    BEGIN_ARTIFACT_UPLOAD_TOOL, GET_BUDGET_REPORT_TOOL, LIST_PENDING_BUDGET_RESERVATIONS_TOOL,
};
use super::tool_def;

pub(super) fn insert_budget_tools(tools: &mut Vec<Value>) {
    let artifact_index = tools
        .iter()
        .position(|tool| {
            tool.get("name").and_then(Value::as_str) == Some(BEGIN_ARTIFACT_UPLOAD_TOOL)
        })
        .expect("artifact transfer tools belong to the gRPC catalog");
    tools.splice(
        artifact_index..artifact_index,
        [
            tool_def(
                GET_BUDGET_REPORT_TOOL,
                "Read the durable budget balance shared by one ceremony tree.",
                budget_report_schema(),
            ),
            tool_def(
                LIST_PENDING_BUDGET_RESERVATIONS_TOOL,
                "Read a bounded global recovery page of budget reservations awaiting terminal receipts.",
                pending_budget_reservations_schema(),
            ),
        ],
    );
}
