use made_app::usecases::agentic_system::AgenticSystemValidationView;
use made_core::value_objects::AgenticSystemValidationFinding;
use serde_json::{json, Value};

/// Renders what the analysis found.
///
/// The locus is serialized structurally and said in words as well: a
/// machine branches on the first and a person reads the second, and a
/// report that offered only one of them would be usable by only one
/// of them.
#[derive(Debug, Default, Clone, Copy)]
pub struct AgenticSystemValidationJson;

impl AgenticSystemValidationJson {
    #[must_use]
    pub fn of(view: &AgenticSystemValidationView) -> Value {
        json!({
            "system_id": view.system().id().as_str(),
            "revision": view.system().revision().get(),
            "publishable": view.is_publishable(),
            "error_count": view.error_count(),
            "warning_count": view.warning_count(),
            "findings": view.findings().iter().map(Self::finding).collect::<Vec<_>>(),
            "resolved_pins": view
                .resolved_pins()
                .iter()
                .map(|(ceremony, pin)| {
                    json!({
                        "ceremony": ceremony.as_str(),
                        "name": pin.name().as_str(),
                        "version": pin.version().as_str(),
                        "digest": pin.digest().to_hex(),
                    })
                })
                .collect::<Vec<_>>(),
        })
    }

    #[must_use]
    pub fn finding(finding: &AgenticSystemValidationFinding) -> Value {
        json!({
            "severity": if finding.is_blocking() { "error" } else { "warning" },
            "locus": serde_json::to_value(finding.locus()).unwrap_or(Value::Null),
            "where": finding.locus().to_string(),
            "message": finding.defect().to_string(),
        })
    }
}
