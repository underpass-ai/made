//! The published catalogue, read in process.
//!
//! Answers in the keys the gRPC backend's projection writes, so a
//! client that switches backends reads the same words about the same
//! publication.

use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::protocol::{ToolError, GET_CEREMONY_DEFINITION_TOOL, LIST_CEREMONY_DEFINITIONS_TOOL};

mod presenter;
mod requests;

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        LIST_CEREMONY_DEFINITIONS_TOOL | GET_CEREMONY_DEFINITION_TOOL
    )
}

pub(super) async fn dispatch(
    made: &EmbeddedMade,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    match name {
        LIST_CEREMONY_DEFINITIONS_TOOL => {
            let page = made.list_definitions(&requests::query(arguments)?).await?;
            Ok(presenter::page(&page))
        }
        GET_CEREMONY_DEFINITION_TOOL => {
            let (name, version) = requests::identity(arguments)?;
            let published = made.get_definition(&name, &version).await?;
            presenter::definition(&published)
        }
        _ => Err(ToolError::invalid_request(format!("unknown tool {name}"))),
    }
}
