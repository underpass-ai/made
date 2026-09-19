use serde_json::Value;

use super::super::ceremony_schemas::{
    get_ceremony_agent_schema, list_ceremony_agents_schema, report_ceremony_agent_status_schema,
};
use super::super::schema_primitives::tool_def;
use super::super::tool_names::{
    GET_CEREMONY_AGENT_TOOL, LIST_CEREMONY_AGENTS_TOOL, REPORT_CEREMONY_AGENT_STATUS_TOOL,
};

pub(super) fn ceremony_agent_tool_catalog() -> [Value; 3] {
    [
        tool_def(
            LIST_CEREMONY_AGENTS_TOOL,
            "List a bounded, authorized ceremony-agent roster. Execution state is separate from fresh/stale/unreachable liveness and host disappearance is not fabricated as success or failure.",
            list_ceremony_agents_schema(),
        ),
        tool_def(
            GET_CEREMONY_AGENT_TOOL,
            "Get one authorized logical ceremony-agent execution with host/incarnation provenance and bounded status evidence.",
            get_ceremony_agent_schema(),
        ),
        tool_def(
            REPORT_CEREMONY_AGENT_STATUS_TOOL,
            "Report authenticated host status for an accepted ceremony claim. Reports are ordered, idempotent and fenced; summaries must not include chain-of-thought or secrets.",
            report_ceremony_agent_status_schema(),
        ),
    ]
}
