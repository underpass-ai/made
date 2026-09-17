use made_app::usecases::{GenerateCeremonyReportInput, ReportTitle};
use made_core::value_objects::CeremonyId;
use serde_json::Value;

use super::embedded_request_fields::required_strings;

/// Validated request for a deterministic, read-only ceremony report.
///
/// Shape only: what a report *is* — the sessions it reads, the
/// document it renders, and what it refuses to report on — belongs to
/// `GenerateCeremonyReportUseCase`, which both editions call.
#[derive(Debug)]
pub(super) struct EmbeddedGenerateCeremonyReportRequest {
    ceremony_ids: Vec<CeremonyId>,
    title: Option<ReportTitle>,
}

impl EmbeddedGenerateCeremonyReportRequest {
    pub(super) fn into_input(self) -> GenerateCeremonyReportInput {
        GenerateCeremonyReportInput::new(self.ceremony_ids, self.title)
    }
}

impl TryFrom<&Value> for EmbeddedGenerateCeremonyReportRequest {
    type Error = String;

    fn try_from(arguments: &Value) -> Result<Self, Self::Error> {
        let object = arguments
            .as_object()
            .ok_or_else(|| "tool arguments must be an object".to_owned())?;
        let ceremony_ids = required_strings(object, "ceremony_ids")?
            .iter()
            .enumerate()
            .map(|(index, raw)| {
                CeremonyId::new(raw)
                    .map_err(|error| format!("invalid ceremony_ids[{index}]: {error}"))
            })
            .collect::<Result<Vec<_>, String>>()?;
        // Built here rather than trimmed here: the trim rule and the
        // blank rule are the value object's, so this arm and the gRPC
        // server apply one rule instead of two.
        let title = object
            .get("title")
            .map(|value| {
                let raw = value
                    .as_str()
                    .ok_or_else(|| "field `title` must be a string".to_owned())?;
                ReportTitle::new(raw).map_err(|error| error.to_string())
            })
            .transpose()?;

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
