use made_app::services::{AuthorizationOperationScope, CeremonyTraceScope};
use made_core::value_objects::{AuthorizationRequestId, TraceContext};
use serde_json::Value;

use super::{embedded_authorization_dispatch, EmbeddedMadeMcpBackend};
use crate::backend::{MadeMcpToolBackend, MadeMcpToolFuture, ToolTraceContext};
use crate::protocol::{tool_success_result, ToolError, SEARCH_CEREMONY_INSTANCES_TOOL};

pub(super) fn call<'a>(
    backend: &'a EmbeddedMadeMcpBackend,
    name: &'a str,
    arguments: &'a Value,
    trace: &'a ToolTraceContext,
) -> MadeMcpToolFuture<'a> {
    Box::pin(async move {
        let authorization_request_id =
            AuthorizationRequestId::new(trace.authorization_request_id().to_owned())
                .map_err(|error| ToolError::invalid_request(error.to_string()))?;
        let trace_context = TraceContext::parse(trace.traceparent())
            .map_err(|error| ToolError::invalid_request(error.to_string()))?;
        let dispatch = async {
            let Some(authorization) = &backend.authorization else {
                return backend.call_tool(name, arguments).await;
            };
            if name == "made_approve_authorization_operation" {
                let decision = authorization.approve_operation(arguments, trace).await?;
                return embedded_authorization_dispatch::present_approval(&decision)
                    .map(tool_success_result);
            }
            let operation = authorization.authorize(name, arguments, trace).await?;
            AuthorizationOperationScope::run(operation, async {
                if name == SEARCH_CEREMONY_INSTANCES_TOOL {
                    return backend
                        .search_and_present(arguments, authorization_request_id)
                        .await;
                }
                backend.call_tool(name, arguments).await
            })
            .await
        };
        CeremonyTraceScope::run(trace_context, dispatch).await
    })
}
