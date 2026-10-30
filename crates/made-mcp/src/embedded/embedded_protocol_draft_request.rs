use made_adapters::yaml::ProtocolDefinitionYaml;
use made_core::entities::ProtocolDefinitionDraft;
use serde_json::Value;

use super::embedded_request_fields::required_string;

use crate::mcp_protocol::ToolError;

/// Validated MCP request carrying a protocol draft to analyse.
///
/// Authoring is read-only: the request holds the draft and nothing
/// else, and answering it never touches MADE.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EmbeddedProtocolDraftRequest {
    definition_yaml: String,
}

impl EmbeddedProtocolDraftRequest {
    /// Parse the draft.
    ///
    /// A failure here is a syntax or identifier failure, not a
    /// structural one: a draft that parses is always analysable, even
    /// when it could never be published.
    pub(crate) fn parse(&self) -> Result<ProtocolDefinitionDraft, ToolError> {
        ProtocolDefinitionYaml::parse_draft_str(&self.definition_yaml).map_err(ToolError::from)
    }
}

impl TryFrom<&Value> for EmbeddedProtocolDraftRequest {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_missing_definition_is_rejected() {
        let error = EmbeddedProtocolDraftRequest::try_from(&json!({})).unwrap_err();

        assert!(error.contains("definition_yaml"), "{error}");
    }

    #[test]
    fn an_unpublishable_draft_still_parses() {
        let request = EmbeddedProtocolDraftRequest::try_from(&json!({
            "definition_yaml": UNPUBLISHABLE,
        }))
        .unwrap();

        let draft = request.parse().expect("a parseable draft");

        assert!(!draft.analyze().is_valid());
    }

    const UNPUBLISHABLE: &str = r#"
version: "1.0"
name: "broken_protocol"
inputs:
  required: []
  optional: []
outputs: {}
states:
  - id: DRAFTING
    initial: true
    terminal: false
  - id: DONE
    initial: false
    terminal: true
transitions:
  - from: DRAFTING
    to: NOWHERE
    trigger: "finish"
    guards: []
steps: []
roles: []
"#;
}
