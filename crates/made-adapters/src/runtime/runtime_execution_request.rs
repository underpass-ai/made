use std::collections::BTreeMap;

use made_core::entities::Proposal;
use made_core::error::DomainError;
use made_core::value_objects::Attributes;
use made_proto::runtime_v1 as runtime_pb;
use prost_types::{value::Kind as PbKind, ListValue, Struct as PbStruct, Value as PbValue};
use serde_json::{Map as JsonMap, Value as JsonValue};

use super::RuntimePrincipal;

pub(super) const KEY_TOOL_NAME: &str = "runtime.tool_name";
pub(super) const KEY_ARGS: &str = "runtime.args";
pub(super) const KEY_APPROVED: &str = "runtime.approved";
pub(super) const KEY_CORRELATION_ID: &str = "runtime.correlation_id";
pub(super) const KEY_SESSION_ID: &str = "runtime.session_id";
pub(super) const KEY_SESSION_REQUESTED_ID: &str = "runtime.session.requested_id";
pub(super) const KEY_SESSION_REPO_URL: &str = "runtime.session.repo_url";
pub(super) const KEY_SESSION_REPO_REF: &str = "runtime.session.repo_ref";
pub(super) const KEY_SESSION_SOURCE_REPO_PATH: &str = "runtime.session.source_repo_path";
pub(super) const KEY_SESSION_ALLOWED_PATHS: &str = "runtime.session.allowed_paths";
pub(super) const KEY_SESSION_METADATA: &str = "runtime.session.metadata";
pub(super) const KEY_SESSION_EXPIRES_IN_SECONDS: &str = "runtime.session.expires_in_seconds";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeExecutionRequest {
    pub(super) session_id: Option<String>,
    requested_session_id: Option<String>,
    repo_url: Option<String>,
    repo_ref: Option<String>,
    source_repo_path: Option<String>,
    allowed_paths: Vec<String>,
    session_metadata: BTreeMap<String, String>,
    expires_in_seconds: Option<u32>,
    principal: RuntimePrincipal,
    pub(super) tool_name: String,
    correlation_id: String,
    pub(super) approved: bool,
    args: JsonMap<String, JsonValue>,
}

impl RuntimeExecutionRequest {
    pub(super) fn from_inputs(
        winner: &Proposal,
        options: &Attributes,
        default_principal: &RuntimePrincipal,
    ) -> Result<Self, DomainError> {
        let tool_name = required_string_value(options, winner, KEY_TOOL_NAME)?;
        let correlation_id = optional_string_value(options, winner, KEY_CORRELATION_ID)?
            .unwrap_or_else(|| winner.id().as_str().to_owned());
        let approved = optional_bool_value(options, winner, KEY_APPROVED)?.unwrap_or(false);
        let session_id = optional_string_value(options, winner, KEY_SESSION_ID)?;
        let requested_session_id =
            optional_string_value(options, winner, KEY_SESSION_REQUESTED_ID)?;
        let repo_url = optional_string_value(options, winner, KEY_SESSION_REPO_URL)?;
        let repo_ref = optional_string_value(options, winner, KEY_SESSION_REPO_REF)?;
        let source_repo_path =
            optional_string_value(options, winner, KEY_SESSION_SOURCE_REPO_PATH)?;
        let allowed_paths =
            optional_string_array(options, winner, KEY_SESSION_ALLOWED_PATHS)?.unwrap_or_default();
        let expires_in_seconds =
            optional_u32_value(options, winner, KEY_SESSION_EXPIRES_IN_SECONDS)?;
        let args = optional_object_value(options, winner, KEY_ARGS)?.unwrap_or_default();

        let mut session_metadata =
            optional_string_map(options, winner, KEY_SESSION_METADATA)?.unwrap_or_default();
        session_metadata
            .entry("made.proposal_id".to_owned())
            .or_insert_with(|| winner.id().as_str().to_owned());
        session_metadata
            .entry("made.author_id".to_owned())
            .or_insert_with(|| winner.author().as_str().to_owned());
        session_metadata
            .entry("made.specialty".to_owned())
            .or_insert_with(|| winner.specialty().as_str().to_owned());

        Ok(Self {
            session_id,
            requested_session_id,
            repo_url,
            repo_ref,
            source_repo_path,
            allowed_paths,
            session_metadata,
            expires_in_seconds,
            principal: default_principal.clone(),
            tool_name,
            correlation_id,
            approved,
            args,
        })
    }

    pub(super) fn create_session_request(&self) -> runtime_pb::CreateSessionRequest {
        runtime_pb::CreateSessionRequest {
            session_id: self.requested_session_id.clone().unwrap_or_default(),
            repo_url: self.repo_url.clone().unwrap_or_default(),
            repo_ref: self.repo_ref.clone().unwrap_or_default(),
            source_repo_path: self.source_repo_path.clone().unwrap_or_default(),
            allowed_paths: self.allowed_paths.clone(),
            principal: Some(self.principal.to_proto()),
            metadata: self.session_metadata.clone().into_iter().collect(),
            expires_in_seconds: self
                .expires_in_seconds
                .and_then(|value| i32::try_from(value).ok())
                .unwrap_or_default(),
        }
    }

    pub(super) fn invoke_tool_request(&self, session_id: String) -> runtime_pb::InvokeToolRequest {
        runtime_pb::InvokeToolRequest {
            session_id,
            tool_name: self.tool_name.clone(),
            correlation_id: self.correlation_id.clone(),
            args: Some(PbStruct {
                fields: self
                    .args
                    .iter()
                    .map(|(k, v)| (k.clone(), json_to_pb_value(v)))
                    .collect(),
            }),
            approved: self.approved,
        }
    }
}

fn required_string_value(
    options: &Attributes,
    winner: &Proposal,
    key: &'static str,
) -> Result<String, DomainError> {
    optional_string_value(options, winner, key)?.ok_or(DomainError::EmptyField { field: key })
}

fn optional_string_value(
    options: &Attributes,
    winner: &Proposal,
    key: &'static str,
) -> Result<Option<String>, DomainError> {
    match lookup_value(options, winner, key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::String(value)) if value.trim().is_empty() => Ok(None),
        Some(JsonValue::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(DomainError::InvariantViolated {
            reason: "runtime string option must be a string",
        }),
    }
}

fn optional_bool_value(
    options: &Attributes,
    winner: &Proposal,
    key: &'static str,
) -> Result<Option<bool>, DomainError> {
    match lookup_value(options, winner, key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(DomainError::InvariantViolated {
            reason: "runtime bool option must be a bool",
        }),
    }
}

fn optional_u32_value(
    options: &Attributes,
    winner: &Proposal,
    key: &'static str,
) -> Result<Option<u32>, DomainError> {
    match lookup_value(options, winner, key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::Number(value)) => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or(DomainError::InvariantViolated {
                reason: "runtime numeric option must be a non-negative integer",
            }),
        Some(_) => Err(DomainError::InvariantViolated {
            reason: "runtime numeric option must be a non-negative integer",
        }),
    }
}

fn optional_string_array(
    options: &Attributes,
    winner: &Proposal,
    key: &'static str,
) -> Result<Option<Vec<String>>, DomainError> {
    match lookup_value(options, winner, key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::Array(values)) => values
            .iter()
            .map(|value| match value {
                JsonValue::String(item) if !item.trim().is_empty() => Ok(item.clone()),
                _ => Err(DomainError::InvariantViolated {
                    reason: "runtime string array option must be an array of strings",
                }),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(DomainError::InvariantViolated {
            reason: "runtime string array option must be an array of strings",
        }),
    }
}

fn optional_string_map(
    options: &Attributes,
    winner: &Proposal,
    key: &'static str,
) -> Result<Option<BTreeMap<String, String>>, DomainError> {
    match lookup_value(options, winner, key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::Object(values)) => values
            .iter()
            .map(|(field, value)| match value {
                JsonValue::String(text) => Ok((field.clone(), text.clone())),
                _ => Err(DomainError::InvariantViolated {
                    reason: "runtime metadata must be an object of strings",
                }),
            })
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(Some),
        Some(_) => Err(DomainError::InvariantViolated {
            reason: "runtime metadata must be an object of strings",
        }),
    }
}

fn optional_object_value(
    options: &Attributes,
    winner: &Proposal,
    key: &'static str,
) -> Result<Option<JsonMap<String, JsonValue>>, DomainError> {
    match lookup_value(options, winner, key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::Object(value)) => Ok(Some(value.clone())),
        Some(_) => Err(DomainError::InvariantViolated {
            reason: "runtime args must be a JSON object",
        }),
    }
}

fn lookup_value<'a>(
    options: &'a Attributes,
    winner: &'a Proposal,
    key: &str,
) -> Option<&'a JsonValue> {
    options.get(key).or_else(|| winner.attributes().get(key))
}

pub(super) fn json_to_pb_value(value: &JsonValue) -> PbValue {
    let kind = match value {
        JsonValue::Null => PbKind::NullValue(0),
        JsonValue::Bool(boolean) => PbKind::BoolValue(*boolean),
        JsonValue::Number(number) => PbKind::NumberValue(number.as_f64().unwrap_or_default()),
        JsonValue::String(text) => PbKind::StringValue(text.clone()),
        JsonValue::Array(values) => PbKind::ListValue(ListValue {
            values: values.iter().map(json_to_pb_value).collect(),
        }),
        JsonValue::Object(object) => PbKind::StructValue(PbStruct {
            fields: object
                .iter()
                .map(|(key, value)| (key.clone(), json_to_pb_value(value)))
                .collect(),
        }),
    };
    PbValue { kind: Some(kind) }
}
