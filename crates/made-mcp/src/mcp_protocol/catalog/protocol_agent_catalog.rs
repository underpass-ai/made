use serde_json::Value;

use super::super::protocol_schemas::{
    get_protocol_agent_schema, list_protocol_agents_schema, report_protocol_agent_status_schema,
};
use super::super::schema_primitives::tool_def;
use super::super::tool_names::{
    GET_PROTOCOL_AGENT_TOOL, LIST_PROTOCOL_AGENTS_TOOL, REPORT_PROTOCOL_AGENT_STATUS_TOOL,
};

pub(super) fn protocol_agent_tool_catalog() -> [Value; 3] {
    [
        tool_def(
            LIST_PROTOCOL_AGENTS_TOOL,
            "List a bounded, authorized protocol-agent roster. Execution state is separate from fresh/stale/unreachable liveness and host disappearance is not fabricated as success or failure.",
            list_protocol_agents_schema(),
        ),
        tool_def(
            GET_PROTOCOL_AGENT_TOOL,
            "Get one authorized logical protocol-agent execution with host/incarnation provenance and bounded status evidence.",
            get_protocol_agent_schema(),
        ),
        tool_def(
            REPORT_PROTOCOL_AGENT_STATUS_TOOL,
            "Report authenticated host status for an accepted protocol claim. Reports are ordered, idempotent and fenced; summaries must not include chain-of-thought or secrets.",
            report_protocol_agent_status_schema(),
        ),
    ]
}
