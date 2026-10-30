//! The catalogue entries for the loop a host drives a whole scope by.

use serde_json::Value;

use crate::mcp_protocol::protocol_schemas::{
    acknowledge_integrator_attention_schema, await_integrator_attention_schema,
    bind_protocol_integrator_schema, get_protocol_integrator_binding_schema,
    list_attention_deliveries_schema,
};
use crate::mcp_protocol::schema_primitives::tool_def;
use crate::mcp_protocol::tool_names::{
    ACKNOWLEDGE_INTEGRATOR_ATTENTION_TOOL, AWAIT_INTEGRATOR_ATTENTION_TOOL,
    BIND_PROTOCOL_INTEGRATOR_TOOL, GET_PROTOCOL_INTEGRATOR_BINDING_TOOL,
    LIST_ATTENTION_DELIVERIES_TOOL,
};

pub(super) fn integrator_loop_tool_catalog() -> Vec<Value> {
    vec![
        tool_def(
            BIND_PROTOCOL_INTEGRATOR_TOOL,
            "Put one host in charge of one protocol or one system run. Answers with the binding and the fence every later call is checked against; a live binding is refused rather than displaced unless you ask for it.",
            bind_protocol_integrator_schema(),
        ),
        tool_def(
            GET_PROTOCOL_INTEGRATOR_BINDING_TOOL,
            "Read who is driving a scope now, under which incarnation and fence. Null means nobody is.",
            get_protocol_integrator_binding_schema(),
        ),
        tool_def(
            AWAIT_INTEGRATOR_ATTENTION_TOOL,
            "Be handed whatever this integrator is owed, holding the line for a bounded while when there is nothing yet. Every answer carries the loop state, the empty ones included: read the protocol again before acting, and stop when it is completed, failed, blocked or awaiting a human decision.",
            await_integrator_attention_schema(),
        ),
        tool_def(
            ACKNOWLEDGE_INTEGRATOR_ATTENTION_TOOL,
            "Say what you are about to do about one item, or what you did, or that you could not. Intent and effect are two calls in that order, so a crash between them can be told from an effect that never started.",
            acknowledge_integrator_attention_schema(),
        ),
        tool_def(
            LIST_ATTENTION_DELIVERIES_TOOL,
            "Read the loop's paperwork: what has been offered to which binding, how often, and where each offer stands. Delivery to a host is transport, never evidence that anybody acted.",
            list_attention_deliveries_schema(),
        ),
    ]
}
