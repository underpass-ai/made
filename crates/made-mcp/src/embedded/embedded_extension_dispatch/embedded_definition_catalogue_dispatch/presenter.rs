//! The published catalogue as JSON, in the keys the gRPC backend's
//! projection writes.

use made_adapters::yaml::PublishedCeremonyDefinitionYaml;
use made_app::usecases::PublishedCeremonyDefinitionPage;
use made_core::entities::PublishedCeremonyDefinition;
use serde_json::{json, Value};

use crate::protocol::ToolError;

pub(super) fn page(page: &PublishedCeremonyDefinitionPage) -> Value {
    json!({
        "definitions": page.definitions().iter().map(summary).collect::<Vec<_>>(),
        "next_cursor": page.next_cursor().map(ToString::to_string),
    })
}

pub(super) fn definition(published: &PublishedCeremonyDefinition) -> Result<Value, ToolError> {
    Ok(json!({
        "ceremony": published.name().as_str(),
        "version": published.version().as_str(),
        "digest": published.digest().to_hex(),
        "definition_yaml": PublishedCeremonyDefinitionYaml::render(published)?,
    }))
}

fn summary(published: &PublishedCeremonyDefinition) -> Value {
    let definition = published.definition();
    json!({
        "ceremony": published.name().as_str(),
        "version": published.version().as_str(),
        "digest": published.digest().to_hex(),
        "description": definition
            .description()
            .map(made_core::value_objects::CeremonyDescription::as_str),
        "state_count": definition.states().len(),
        "step_count": definition.steps().len(),
    })
}
