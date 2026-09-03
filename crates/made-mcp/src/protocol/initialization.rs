use serde_json::{json, Value};

use super::PROTOCOL_VERSION;

/// Build the `initialize` result. Includes adapter-side metadata so
/// the client can record which backend + TLS posture it negotiated
/// without an extra round-trip.
pub(crate) fn initialize_result(
    server_name: &str,
    server_version: &str,
    backend: &str,
    grpc_tls: &str,
) -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": server_name,
            "version": server_version,
        },
        "metadata": {
            "backend": backend,
            "grpc_tls": grpc_tls,
        }
    })
}
