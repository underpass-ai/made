use std::collections::BTreeSet;

use choreo_core::value_objects::CeremonyId;
use serde_json::Value;

use super::embedded_request_fields::{optional_string, required_strings};

/// Validated request for a deterministic, read-only ceremony report.
#[derive(Debug)]
pub(super) struct EmbeddedGenerateCeremonyReportRequest {
    ceremony_ids: Vec<CeremonyId>,
    title: Option<String>,
}

impl EmbeddedGenerateCeremonyReportRequest {
    pub(super) fn ceremony_ids(&self) -> &[CeremonyId] {
        &self.ceremony_ids
    }

    pub(super) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

impl TryFrom<&Value> for EmbeddedGenerateCeremonyReportRequest {
    type Error = String;

    fn try_from(arguments: &Value) -> Result<Self, Self::Error> {
        let object = arguments
            .as_object()
            .ok_or_else(|| "tool arguments must be an object".to_owned())?;
        let values = required_strings(object, "ceremony_ids")?;
        if values.is_empty() {
            return Err("field `ceremony_ids` must contain at least one id".to_owned());
        }

        let mut seen = BTreeSet::new();
        let ceremony_ids = values
            .iter()
            .enumerate()
            .map(|(index, raw)| {
                let id = CeremonyId::new(raw)
                    .map_err(|error| format!("invalid ceremony_ids[{index}]: {error}"))?;
                if !seen.insert(id.clone()) {
                    return Err(format!("duplicate ceremony id `{id}`"));
                }
                Ok(id)
            })
            .collect::<Result<Vec<_>, String>>()?;

        let title = optional_string(object, "title")?;
        if title.as_ref().is_some_and(|value| value.trim().is_empty()) {
            return Err("field `title` must not be empty when supplied".to_owned());
        }

        // Reject unknown fields here even when the caller did not obtain the schema first.
        for field in object.keys() {
            if field != "ceremony_ids" && field != "title" {
                return Err(format!("unknown field `{field}`"));
            }
        }

        Ok(Self {
            ceremony_ids,
            title,
        })
    }
}
