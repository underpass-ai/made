//! The published catalogue as JSON, in the keys the gRPC backend's
//! projection writes.

use made_adapters::yaml::PublishedProtocolDefinitionYaml;
use made_app::usecases::PublishedProtocolDefinitionPage;
use made_core::entities::{ProtocolCatalogueEntry, PublishedProtocolDefinition};
use serde_json::{json, Value};

use crate::mcp_protocol::ToolError;

pub(super) fn page(page: &PublishedProtocolDefinitionPage) -> Value {
    json!({
        "definitions": page.entries().iter().map(summary).collect::<Vec<_>>(),
        "next_cursor": page.next_cursor().map(ToString::to_string),
    })
}

pub(super) fn definition(published: &PublishedProtocolDefinition) -> Result<Value, ToolError> {
    Ok(json!({
        "protocol": published.name().as_str(),
        "version": published.version().as_str(),
        "digest": published.digest().to_hex(),
        "definition_yaml": PublishedProtocolDefinitionYaml::render(published)?,
    }))
}

fn summary(entry: &ProtocolCatalogueEntry) -> Value {
    match entry {
        ProtocolCatalogueEntry::Readable(published) => {
            let definition = published.definition();
            json!({
                "protocol": published.name().as_str(),
                "version": published.version().as_str(),
                "digest": published.digest().to_hex(),
                "description": definition
                    .description()
                    .map(made_core::value_objects::ProtocolDescription::as_str),
                "state_count": definition.states().len(),
                "step_count": definition.steps().len(),
                "unreadable": null,
            })
        }
        ProtocolCatalogueEntry::Unreadable(unreadable) => json!({
            "protocol": unreadable.name().as_str(),
            "version": unreadable.version().as_str(),
            "digest": unreadable
                .recorded_digest()
                .map(made_core::value_objects::ProtocolDefinitionDigest::to_hex),
            "description": null,
            "state_count": null,
            "step_count": null,
            "unreadable": unreadable.defect().to_string(),
        }),
    }
}
