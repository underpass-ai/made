use made_app::usecases::ProtocolDefinitionSource;
use made_core::value_objects::{ProtocolDefinitionDiff, ProtocolName, ProtocolVersion};
use made_embedded::EmbeddedMade;
use serde_json::Map;
use serde_json::Value;

use made_adapters::yaml::ProtocolDefinitionYaml;

use super::embedded_request_fields::optional_string;

use crate::mcp_protocol::ToolError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedDiffProtocolDefinitionsRequest {
    before: ProtocolDefinitionSource,
    after: ProtocolDefinitionSource,
}

impl EmbeddedDiffProtocolDefinitionsRequest {
    pub(super) async fn execute(
        self,
        made: &EmbeddedMade,
    ) -> Result<ProtocolDefinitionDiff, ToolError> {
        made.diff_definitions(self.before, self.after)
            .await
            .map_err(ToolError::from)
    }
}

impl TryFrom<&Value> for EmbeddedDiffProtocolDefinitionsRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = as_object(value, "tools/call.arguments")?;
        Ok(Self {
            before: source(object.get("before"), "before")?,
            after: source(object.get("after"), "after")?,
        })
    }
}

/// Either a published version, named, or a document supplied for the
/// occasion. Both at once has no reading, and neither leaves nothing
/// to compare.
fn source(value: Option<&Value>, side: &str) -> Result<ProtocolDefinitionSource, String> {
    let value = value.ok_or_else(|| format!("missing required object `{side}`"))?;
    let object = as_object(value, side)?;
    let protocol = optional_string(object, "protocol")?.filter(|value| !value.trim().is_empty());
    let version = optional_string(object, "version")?.filter(|value| !value.trim().is_empty());
    let yaml = optional_string(object, "definition_yaml")?.filter(|value| !value.trim().is_empty());
    let names_a_version = protocol.is_some() || version.is_some();

    if names_a_version && yaml.is_some() {
        return Err(format!(
            "`{side}` is either a published version or a supplied definition, not both"
        ));
    }
    if let Some(yaml) = yaml {
        return Ok(ProtocolDefinitionSource::supplied(
            ProtocolDefinitionYaml::parse_str(&yaml)
                .map_err(|error| format!("invalid definition in `{side}`: {error}"))?,
        ));
    }
    match (protocol, version) {
        (Some(protocol), Some(version)) => Ok(ProtocolDefinitionSource::published(
            ProtocolName::new(protocol).map_err(|error| error.to_string())?,
            ProtocolVersion::new(version).map_err(|error| error.to_string())?,
        )),
        (None, None) => Err(format!(
            "`{side}` must name a published definition or supply one"
        )),
        // Half a coordinate names nothing, and picking a version on
        // the caller's behalf would answer a different question.
        _ => Err(format!(
            "`{side}` names a published definition, so it needs both `protocol` and `version`"
        )),
    }
}

fn as_object<'a>(value: &'a Value, where_: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("`{where_}` must be an object"))
}
