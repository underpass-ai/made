//! The nine agentic-system tools, in process.
//!
//! Answers are rendered by the same projection the gRPC backend uses,
//! so a client that switches backends reads the same words about the
//! same design rather than two renderings that happen to agree today.

use made_adapters::json::{
    AgenticSystemExecutionJson, AgenticSystemJson, AgenticSystemValidationJson,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::protocol::{
    ToolError, ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL, DESIGN_AGENTIC_SYSTEM_TOOL,
    GET_AGENTIC_SYSTEM_EXECUTION_TOOL, GET_AGENTIC_SYSTEM_TOOL, INSTANTIATE_AGENTIC_SYSTEM_TOOL,
    LIST_AGENTIC_SYSTEMS_TOOL, PUBLISH_AGENTIC_SYSTEM_TOOL, RENDER_AGENTIC_SYSTEM_DIAGRAM_TOOL,
    VALIDATE_AGENTIC_SYSTEM_TOOL,
};

mod requests;

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        DESIGN_AGENTIC_SYSTEM_TOOL
            | GET_AGENTIC_SYSTEM_TOOL
            | LIST_AGENTIC_SYSTEMS_TOOL
            | VALIDATE_AGENTIC_SYSTEM_TOOL
            | PUBLISH_AGENTIC_SYSTEM_TOOL
            | INSTANTIATE_AGENTIC_SYSTEM_TOOL
            | ADVANCE_AGENTIC_SYSTEM_EXECUTION_TOOL
            | GET_AGENTIC_SYSTEM_EXECUTION_TOOL
            | RENDER_AGENTIC_SYSTEM_DIAGRAM_TOOL
    )
}

pub(super) async fn dispatch(
    made: &EmbeddedMade,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    match name {
        "made_design_agentic_system" => {
            let view = made
                .design_agentic_system(requests::design_document(arguments)?)
                .await?;
            Ok(AgenticSystemJson::of(&view)?)
        }
        "made_get_agentic_system" => {
            let view = made
                .get_agentic_system(
                    &requests::system_id(arguments)?,
                    requests::revision(arguments)?,
                )
                .await?;
            Ok(AgenticSystemJson::of(&view)?)
        }
        "made_list_agentic_systems" => {
            let page = made
                .list_agentic_systems(&requests::list_query(arguments)?)
                .await?;
            Ok(AgenticSystemJson::page(&page)?)
        }
        "made_validate_agentic_system" => {
            let view = made
                .validate_agentic_system(
                    &requests::system_id(arguments)?,
                    requests::revision(arguments)?,
                )
                .await?;
            Ok(AgenticSystemValidationJson::of(&view))
        }
        "made_publish_agentic_system" => {
            let view = made
                .publish_agentic_system(
                    &requests::system_id(arguments)?,
                    requests::required_revision(arguments)?,
                )
                .await?;
            Ok(AgenticSystemJson::published(&view))
        }
        "made_instantiate_agentic_system" => {
            let input = requests::instantiate_input(arguments)?;
            let execution = Box::pin(made.instantiate_agentic_system(input)).await?;
            Ok(AgenticSystemExecutionJson::of(&execution))
        }
        "made_advance_agentic_system_execution" => {
            let (actor_id, actor_kind) = requests::actor(arguments)?;
            let execution = Box::pin(made.advance_agentic_system_execution(
                &requests::execution_id(arguments)?,
                &actor_id,
                actor_kind,
            ))
            .await?;
            Ok(AgenticSystemExecutionJson::of(&execution))
        }
        "made_get_agentic_system_execution" => {
            let view = made
                .get_agentic_system_execution(&requests::execution_id(arguments)?)
                .await?;
            Ok(AgenticSystemExecutionJson::view(&view)?)
        }
        "made_render_agentic_system_diagram" => {
            let diagram = made
                .render_agentic_system_diagram(
                    &requests::system_id(arguments)?,
                    requests::revision(arguments)?,
                    requests::optional_execution_id(arguments)?.as_ref(),
                )
                .await?;
            Ok(AgenticSystemJson::diagram(&diagram))
        }
        _ => Err(ToolError::invalid_request(format!("unknown tool {name}"))),
    }
}
