use made_app::usecases::{GenerateProtocolReportInput, ProtocolReportIds, ReportTitle};
use made_core::value_objects::ProtocolId;
use serde_json::Value;

use super::embedded_request_fields::required_strings;

/// Validated request for a deterministic, read-only protocol report.
///
/// Mapping builds the shared list and title values before the use case reads
/// any session; this adapter does not decide their validation rules.
#[derive(Debug)]
pub(super) struct EmbeddedGenerateProtocolReportRequest {
    protocol_ids: ProtocolReportIds,
    title: Option<ReportTitle>,
}

impl EmbeddedGenerateProtocolReportRequest {
    pub(super) fn into_input(self) -> GenerateProtocolReportInput {
        GenerateProtocolReportInput::from_ids(self.protocol_ids, self.title)
    }
}

impl TryFrom<&Value> for EmbeddedGenerateProtocolReportRequest {
    type Error = String;

    fn try_from(arguments: &Value) -> Result<Self, Self::Error> {
        let object = arguments
            .as_object()
            .ok_or_else(|| "tool arguments must be an object".to_owned())?;
        let protocol_ids = required_strings(object, "protocol_ids")?
            .iter()
            .enumerate()
            .map(|(index, raw)| {
                ProtocolId::new(raw)
                    .map_err(|error| format!("invalid protocol_ids[{index}]: {error}"))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let protocol_ids =
            ProtocolReportIds::new(protocol_ids).map_err(|error| error.to_string())?;
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
            if field != "protocol_ids" && field != "title" {
                return Err(format!("unknown field `{field}`"));
            }
        }

        Ok(Self {
            protocol_ids,
            title,
        })
    }
}
