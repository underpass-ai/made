use made_app::usecases::agentic_system::{AgenticSystemCeremonyView, AgenticSystemExecutionView};
use made_core::entities::AgenticSystemExecution;
use made_core::error::DomainError;
use made_core::value_objects::CeremonyEndReason;
use serde_json::{json, Value};

use super::AgenticSystemJson;

/// Renders a run, with what it intended kept apart from what is
/// actually happening.
#[derive(Debug, Default, Clone, Copy)]
pub struct AgenticSystemExecutionJson;

impl AgenticSystemExecutionJson {
    /// A run as instantiation and advancing answer with it: what the
    /// engine recorded, without going back to the instances.
    #[must_use]
    pub fn of(execution: &AgenticSystemExecution) -> Value {
        json!({
            "execution_id": execution.id().as_str(),
            "system_id": execution.system().id().as_str(),
            "revision": execution.system().revision().get(),
            "digest": execution.system().digest().to_hex(),
            "state": execution.state().as_str(),
            "integrator_binding_id": execution
                .integrator_binding()
                .map(|binding| binding.as_str().to_owned()),
            "participants": execution
                .participants()
                .iter()
                .map(|(participant, materialization)| {
                    json!({
                        "participant": participant.as_str(),
                        "bound": materialization.is_bound(),
                        "specialty": materialization
                            .specialty()
                            .map(|specialty| specialty.as_str().to_owned()),
                        "unavailable_because": materialization
                            .reason()
                            .map(|reason| reason.as_str().to_owned()),
                    })
                })
                .collect::<Vec<_>>(),
            "ceremonies": execution
                .ceremonies()
                .iter()
                .map(|(ceremony, link)| {
                    json!({
                        "ceremony": ceremony.as_str(),
                        "pin": {
                            "name": link.pin().name().as_str(),
                            "version": link.pin().version().as_str(),
                            "digest": link.pin().digest().to_hex(),
                        },
                        "planned": link.status().as_str(),
                        "round": link.round().get(),
                        "instance_id": link
                            .instance_id()
                            .map(|instance| instance.as_str().to_owned()),
                        "skipped_because": link
                            .skipped_because()
                            .map(|reason| reason.as_str().to_owned()),
                    })
                })
                .collect::<Vec<_>>(),
        })
    }

    /// A run read back beside the instances it opened.
    pub fn view(view: &AgenticSystemExecutionView) -> Result<Value, DomainError> {
        let mut rendered = Self::of(view.execution());
        let ceremonies: Vec<Value> = view.ceremonies().iter().map(Self::ceremony).collect();
        if let Some(object) = rendered.as_object_mut() {
            object.insert("ceremonies".to_owned(), Value::Array(ceremonies));
            object.insert(
                "system".to_owned(),
                serde_json::to_value(view.system()).map_err(|_| {
                    DomainError::InvariantViolated {
                        reason: "agentic system cannot be rendered as JSON",
                    }
                })?,
            );
            object.insert(
                "yaml".to_owned(),
                Value::String(crate::yaml::AgenticSystemYaml::render(view.system())?),
            );
        }
        Ok(rendered)
    }

    /// One composition: what the design intended, then what the
    /// instance says, in two separate objects so nobody can read one
    /// as the other.
    #[must_use]
    pub fn ceremony(view: &AgenticSystemCeremonyView) -> Value {
        json!({
            "ceremony": view.ceremony().as_str(),
            "pin": {
                "name": view.pin().name().as_str(),
                "version": view.pin().version().as_str(),
                "digest": view.pin().digest().to_hex(),
            },
            "planned": view.planned().as_str(),
            "round": view.round().get(),
            "instance_id": view.instance_id().map(|instance| instance.as_str().to_owned()),
            "skipped_because": view
                .skipped_because()
                .map(|reason| reason.as_str().to_owned()),
            "observed": view.observed_lifecycle().map(|lifecycle| {
                json!({
                    "phase": lifecycle.phase().as_label(),
                    "end_reason": lifecycle.end_reason().map(CeremonyEndReason::as_label),
                    "state": view.observed_state().map(|state| state.as_str().to_owned()),
                })
            }),
        })
    }

    /// The same shape the design projection uses, for a response that
    /// carries both.
    pub fn system_of(view: &AgenticSystemExecutionView) -> Result<Value, DomainError> {
        AgenticSystemJson::of(&made_app::usecases::agentic_system::AgenticSystemView::of(
            view.system().clone(),
        )?)
    }
}
