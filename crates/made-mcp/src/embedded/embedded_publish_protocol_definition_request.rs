use made_adapters::yaml::ProtocolDefinitionYaml;
use made_core::entities::PublicationOutcome;
use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::embedded_request_fields::required_string;

use crate::mcp_protocol::ToolError;

/// Validated MCP request to fix a definition to an immutable version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EmbeddedPublishProtocolDefinitionRequest {
    definition_yaml: String,
}

impl EmbeddedPublishProtocolDefinitionRequest {
    /// Parse, prove publishable, then publish.
    ///
    /// The draft is analysed on the way through, so a definition that
    /// could never be executed is refused before it occupies a version
    /// — the point of publishing is that what comes back out is known
    /// good.
    pub(crate) async fn execute(
        self,
        made: &EmbeddedMade,
    ) -> Result<PublicationOutcome, ToolError> {
        let draft = ProtocolDefinitionYaml::parse_draft_str(&self.definition_yaml)?;
        let definition = draft.publish()?;

        made.publish_definition(definition)
            .await
            .map_err(ToolError::from)
    }
}

impl TryFrom<&Value> for EmbeddedPublishProtocolDefinitionRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        Ok(Self {
            definition_yaml: required_string(object, "definition_yaml")?,
        })
    }
}
