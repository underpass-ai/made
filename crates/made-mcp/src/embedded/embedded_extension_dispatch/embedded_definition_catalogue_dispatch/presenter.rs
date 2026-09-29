//! The published catalogue as JSON, in the keys the gRPC backend's
//! projection writes.

use made_adapters::yaml::PublishedCeremonyDefinitionYaml;
use made_app::usecases::PublishedCeremonyDefinitionPage;
use made_core::entities::{CeremonyCatalogueEntry, PublishedCeremonyDefinition};
use serde_json::{json, Value};

use crate::protocol::ToolError;

pub(super) fn page(page: &PublishedCeremonyDefinitionPage) -> Value {
    json!({
        "definitions": page.entries().iter().map(summary).collect::<Vec<_>>(),
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

fn summary(entry: &CeremonyCatalogueEntry) -> Value {
    match entry {
        CeremonyCatalogueEntry::Readable(published) => {
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
                "unreadable": null,
            })
        }
        CeremonyCatalogueEntry::Unreadable(unreadable) => json!({
            "ceremony": unreadable.name().as_str(),
            "version": unreadable.version().as_str(),
            "digest": unreadable
                .recorded_digest()
                .map(made_core::value_objects::CeremonyDefinitionDigest::to_hex),
            "description": null,
            "state_count": null,
            "step_count": null,
            "unreadable": unreadable.defect().to_string(),
        }),
    }
}
