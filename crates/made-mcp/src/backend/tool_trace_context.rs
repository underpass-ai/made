use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Trace and idempotency identity carried by one MCP tool invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolTraceContext {
    traceparent: String,
    authorization_request_id: String,
}

impl ToolTraceContext {
    pub(crate) fn for_invocation(
        traceparent: Option<&str>,
        process_session_namespace: &str,
        jsonrpc_id: &Value,
        tool_name: &str,
        arguments: &Value,
        explicit_request_namespace: Option<&str>,
    ) -> Result<Self, String> {
        if let Some(value) = explicit_request_namespace {
            validate_explicit_namespace(value)?;
        }
        let namespace = explicit_request_namespace.unwrap_or(process_session_namespace);
        let jsonrpc_id = explicit_request_namespace.is_none().then_some(jsonrpc_id);
        Ok(Self {
            traceparent: validated_traceparent(traceparent),
            authorization_request_id: derive_request_id(
                namespace, jsonrpc_id, tool_name, arguments,
            ),
        })
    }

    #[must_use]
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
    let mut value = value.clone();
    if let Some(object) = value.as_object_mut() {
        object.remove("_meta");
    }
    serde_json::to_string(&value).expect("JSON values always serialize")
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
            Some(" bad")
        )
        .is_err());
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
        )
        .unwrap()
    }
}
