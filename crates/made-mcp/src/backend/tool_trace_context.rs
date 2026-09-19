use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[cfg(feature = "embedded")]
use made_core::value_objects::AuthorizationTargetDigest;

/// Trace and idempotency identity carried by one MCP tool invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolTraceContext {
    traceparent: String,
    authorization_request_id: String,
    approval_decision_id: Option<String>,
}

impl ToolTraceContext {
    pub(crate) fn for_invocation(
        traceparent: Option<&str>,
        process_session_namespace: &str,
        jsonrpc_id: &Value,
        tool_name: &str,
        arguments: &Value,
        explicit_request_namespace: Option<&str>,
        approval_decision_id: Option<&str>,
    ) -> Result<Self, String> {
        if let Some(value) = explicit_request_namespace {
            validate_explicit_namespace(value)?;
        }
        let namespace = explicit_request_namespace.unwrap_or(process_session_namespace);
        let jsonrpc_id = explicit_request_namespace.is_none().then_some(jsonrpc_id);
        let approval_decision_id = approval_decision_id
            .map(validate_decision_id)
            .transpose()?
            .map(str::to_owned);
        Ok(Self {
            traceparent: validated_traceparent(traceparent),
            authorization_request_id: derive_request_id(
                namespace, jsonrpc_id, tool_name, arguments,
            ),
            approval_decision_id,
        })
    }

    #[must_use]
    #[cfg(feature = "grpc")]
    pub(crate) fn for_direct_call(tool_name: &str, arguments: &Value) -> Self {
        let invocation_namespace = Uuid::new_v4().simple().to_string();
        Self {
            traceparent: validated_traceparent(None),
            authorization_request_id: derive_request_id(
                &invocation_namespace,
                None,
                tool_name,
                arguments,
            ),
            approval_decision_id: None,
        }
    }

    #[must_use]
    pub fn traceparent(&self) -> &str {
        &self.traceparent
    }

    #[must_use]
    pub fn authorization_request_id(&self) -> &str {
        &self.authorization_request_id
    }

    #[must_use]
    pub fn approval_decision_id(&self) -> Option<&str> {
        self.approval_decision_id.as_deref()
    }

    /// Digest of the transport-neutral MCP invocation target.
    ///
    /// `_meta` carries transport controls such as retry and tracing identities;
    /// it is deliberately outside the business target. Object keys are sorted
    /// recursively while arrays retain their order, so equivalent JSON objects
    /// produce the same digest on every host.
    #[cfg(feature = "embedded")]
    #[must_use]
    pub fn authorization_target_digest(
        tool_name: &str,
        arguments: &Value,
    ) -> AuthorizationTargetDigest {
        let target = Value::Object(serde_json::Map::from_iter([
            ("tool".to_owned(), Value::String(tool_name.to_owned())),
            ("args".to_owned(), without_transport_meta(arguments)),
        ]));
        AuthorizationTargetDigest::for_bytes(canonical_json(&target).as_bytes())
    }

    /// Lowercase SHA-256 target sent by the authenticated MCP proxy to gRPC.
    #[cfg(feature = "grpc")]
    pub(crate) fn grpc_authorization_target_digest(
        tool_name: &str,
        arguments: &Value,
    ) -> Result<String, String> {
        if tool_name == "made_search_ceremony_instances" {
            return search_authorization_target_digest(arguments);
        }
        let target = Value::Object(serde_json::Map::from_iter([
            ("tool".to_owned(), Value::String(tool_name.to_owned())),
            ("args".to_owned(), without_transport_meta(arguments)),
        ]));
        Ok(format!(
            "{:x}",
            Sha256::digest(canonical_json(&target).as_bytes())
        ))
    }
}

#[cfg(feature = "grpc")]
fn search_authorization_target_digest(arguments: &Value) -> Result<String, String> {
    let object = arguments
        .as_object()
        .ok_or_else(|| "tools/call.arguments: expected an object".to_owned())?;
    let cursor = optional_string(object, "cursor")?;
    let limit = match object.get("limit") {
        None => 50_u64,
        Some(value) => value
            .as_u64()
            .filter(|value| (1..=100).contains(value))
            .ok_or_else(|| "`limit` must be an integer from 1 to 100".to_owned())?,
    };
    let id_prefix = optional_string(object, "id_prefix")?;
    let lifecycle = match object.get("lifecycle") {
        None => 0,
        Some(Value::String(value)) if value == "running" => 1,
        Some(Value::String(value)) if value == "paused" => 2,
        Some(Value::String(value)) if value == "ended" => 3,
        Some(_) => return Err("`lifecycle` must be running, paused or ended".to_owned()),
    };
    let mut bytes = b"underpass.made.search-ceremony-instances.v1\0".to_vec();
    push_optional(&mut bytes, cursor);
    bytes.extend_from_slice(&limit.to_be_bytes());
    push_optional(&mut bytes, id_prefix);
    bytes.push(lifecycle);
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[cfg(feature = "grpc")]
fn optional_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<&'a str>, String> {
    object
        .get(field)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("`{field}` must be a string"))
        })
        .transpose()
}

#[cfg(feature = "grpc")]
fn push_optional(bytes: &mut Vec<u8>, value: Option<&str>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
            bytes.extend_from_slice(value.as_bytes());
        }
    }
}

fn validate_explicit_namespace(value: &str) -> Result<(), String> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > 256
        || value.chars().any(char::is_control)
    {
        return Err(
            "_meta.made_request_id must contain 1 to 256 printable, unpadded bytes".to_owned(),
        );
    }
    Ok(())
}

fn validate_decision_id(value: &str) -> Result<&str, String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("_meta.made_approval_decision_id must be a lowercase sha256 value".to_owned());
    }
    Ok(value)
}

fn derive_request_id(
    namespace: &str,
    jsonrpc_id: Option<&Value>,
    tool_name: &str,
    arguments: &Value,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"made-mcp-v1\0");
    hash_field(&mut digest, namespace.as_bytes());
    hash_field(
        &mut digest,
        jsonrpc_id
            .map_or_else(String::new, canonical_json)
            .as_bytes(),
    );
    hash_field(&mut digest, tool_name.as_bytes());
    hash_field(&mut digest, canonical_json(arguments).as_bytes());
    format!("made-{:x}", digest.finalize())
}

fn canonical_json(value: &Value) -> String {
    serde_json::to_string(&canonical_value(value)).expect("JSON values always serialize")
}

fn without_transport_meta(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(object) = value.as_object_mut() {
        object.remove("_meta");
    }
    value
}

fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_value(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(canonical_value).collect()),
        value => value.clone(),
    }
}

fn hash_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

fn validated_traceparent(value: Option<&str>) -> String {
    value
        .filter(|value| valid_traceparent(value))
        .map_or_else(generate_traceparent, str::to_ascii_lowercase)
}

fn generate_traceparent() -> String {
    let trace_id = Uuid::new_v4().simple();
    let span = Uuid::new_v4().as_u128();
    format!("00-{trace_id}-{:016x}-01", span >> 64)
}

fn valid_traceparent(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    parts.len() == 4
        && parts[0].len() == 2
        && parts[1].len() == 32
        && parts[2].len() == 16
        && parts[3].len() == 2
        && parts
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_hexdigit()))
        && parts[1].bytes().any(|byte| byte != b'0')
        && parts[2].bytes().any(|byte| byte != b'0')
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn request_id_is_stable_for_retry_and_changes_with_the_page() {
        let supplied = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";
        let first = invocation(
            Some(supplied),
            "process-a",
            &json!(7),
            &json!({"limit": 10}),
            None,
        );
        let retry = invocation(
            Some(supplied),
            "process-a",
            &json!(7),
            &json!({"limit": 10}),
            None,
        );
        let next = invocation(
            None,
            "process-a",
            &json!(8),
            &json!({"cursor": "next"}),
            None,
        );
        assert_eq!(first.traceparent(), supplied);
        assert_eq!(
            first.authorization_request_id(),
            retry.authorization_request_id()
        );
        assert_ne!(
            first.authorization_request_id(),
            next.authorization_request_id()
        );
    }

    #[test]
    fn separate_normal_calls_get_separate_request_identities() {
        let first = invocation(None, "process-a", &json!(7), &json!({"limit": 10}), None);
        let second = invocation(None, "process-a", &json!(8), &json!({"limit": 10}), None);
        assert_ne!(
            first.authorization_request_id(),
            second.authorization_request_id()
        );
    }

    #[test]
    fn explicit_namespace_survives_restart_and_remains_payload_bound() {
        let first = invocation(
            None,
            "process-a",
            &json!(1),
            &json!({"limit": 10}),
            Some("retry-7"),
        );
        let restarted = invocation(
            None,
            "process-b",
            &json!(99),
            &json!({"limit": 10}),
            Some("retry-7"),
        );
        let changed = invocation(
            None,
            "process-b",
            &json!(99),
            &json!({"limit": 11}),
            Some("retry-7"),
        );
        assert_eq!(
            first.authorization_request_id(),
            restarted.authorization_request_id()
        );
        assert_ne!(
            restarted.authorization_request_id(),
            changed.authorization_request_id()
        );
        assert!(ToolTraceContext::for_invocation(
            None,
            "process",
            &json!(1),
            "tool",
            &json!({}),
            Some(" bad"),
            None,
        )
        .is_err());
    }

    #[cfg(feature = "embedded")]
    #[test]
    fn target_digest_is_recursive_and_excludes_transport_metadata() {
        let left = json!({
            "nested": {"b": 2, "a": 1},
            "ordered": [2, 1],
            "_meta": {"made_request_id": "retry-a"}
        });
        let right = json!({
            "ordered": [2, 1],
            "nested": {"a": 1, "b": 2},
            "_meta": {"made_request_id": "retry-b"}
        });
        assert_eq!(
            ToolTraceContext::authorization_target_digest("made_test", &left),
            ToolTraceContext::authorization_target_digest("made_test", &right)
        );
        assert_ne!(
            ToolTraceContext::authorization_target_digest("made_test", &left),
            ToolTraceContext::authorization_target_digest(
                "made_test",
                &json!({"nested": {"a": 1, "b": 2}, "ordered": [1, 2]})
            )
        );
    }

    #[cfg(all(feature = "grpc", feature = "embedded"))]
    #[test]
    fn grpc_search_digest_matches_the_application_semantic_digest() {
        use made_app::usecases::SearchCeremonyInstancesInput;
        use made_core::value_objects::{
            CeremonyIdPrefix, CeremonyInstancePageLimit, CeremonyLifecyclePhase,
        };

        let arguments = json!({
            "limit": 12,
            "id_prefix": "team-",
            "lifecycle": "paused"
        });
        let expected = SearchCeremonyInstancesInput::new(
            None,
            CeremonyInstancePageLimit::new(12).unwrap(),
            Some(CeremonyIdPrefix::new("team-").unwrap()),
            Some(CeremonyLifecyclePhase::Paused),
        )
        .authorization_target_digest();

        assert_eq!(
            ToolTraceContext::grpc_authorization_target_digest(
                "made_search_ceremony_instances",
                &arguments,
            )
            .unwrap(),
            expected.as_str()
        );
    }

    fn invocation(
        trace: Option<&str>,
        process: &str,
        id: &Value,
        arguments: &Value,
        explicit: Option<&str>,
    ) -> ToolTraceContext {
        ToolTraceContext::for_invocation(
            trace,
            process,
            id,
            "made_search_ceremony_instances",
            arguments,
            explicit,
            None,
        )
        .unwrap()
    }
}
