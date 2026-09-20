use made_app::usecases::agentic_system::{AgenticSystemPublicationView, AgenticSystemView};
use made_core::entities::AgenticSystem;
use made_core::error::DomainError;
use made_core::ports::AgenticSystemPage;
use made_core::value_objects::AgenticSystemDigest;
use serde_json::{json, Value};

use crate::yaml::AgenticSystemYaml;

/// Renders a design, a catalogue page and a publication outcome.
#[derive(Debug, Default, Clone, Copy)]
pub struct AgenticSystemJson;

impl AgenticSystemJson {
    /// One design, with the document a person edits beside the one a
    /// machine reads.
    pub fn of(view: &AgenticSystemView) -> Result<Value, DomainError> {
        let system = view.system();
        Ok(json!({
            "system_id": system.id().as_str(),
            "revision": view.revision().get(),
            "lifecycle": system.lifecycle().as_str(),
            "digest": view.digest().to_hex(),
            "yaml": AgenticSystemYaml::render(system)?,
            "system": serde_json::to_value(system).map_err(|_| unrenderable())?,
        }))
    }

    /// A catalogue page: what each design is, without the document.
    ///
    /// A listing that carried every design in full would be a listing
    /// nobody could afford to call.
    pub fn page(page: &AgenticSystemPage) -> Result<Value, DomainError> {
        Ok(json!({
            "systems": page
                .systems()
                .iter()
                .map(Self::summary)
                .collect::<Result<Vec<_>, _>>()?,
            "next_cursor": page.next_cursor().map(|cursor| cursor.as_str().to_owned()),
        }))
    }

    /// What was sealed, and where the design's history now stands.
    #[must_use]
    pub fn published(view: &AgenticSystemPublicationView) -> Value {
        json!({
            "outcome": view.outcome().as_str(),
            "system_id": view.validation().system().id().as_str(),
            "sealed_revision": view.sealed_revision().get(),
            "head_revision": view.head_revision().get(),
            "digest": view.digest().map(AgenticSystemDigest::to_hex),
        })
    }

    /// The topology, drawn and said.
    #[must_use]
    pub fn diagram(diagram: &made_core::ports::AgenticSystemDiagram) -> Value {
        json!({
            "mermaid": diagram.mermaid(),
            "text_equivalent": diagram.text_equivalent(),
        })
    }

    /// What a design is, without the document it is written in.
    pub fn summary(system: &AgenticSystem) -> Result<Value, DomainError> {
        Ok(json!({
            "system_id": system.id().as_str(),
            "revision": system.revision().get(),
            "lifecycle": system.lifecycle().as_str(),
            "digest": system.digest()?.to_hex(),
            "purpose": system.purpose().as_str(),
        }))
    }
}

fn unrenderable() -> DomainError {
    DomainError::InvariantViolated {
        reason: "agentic system cannot be rendered as JSON",
    }
}
