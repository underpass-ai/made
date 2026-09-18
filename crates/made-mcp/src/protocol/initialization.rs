use base64::Engine;
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
    let mut result = json!({
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
    });
    // Host-owned identities keep their own branding. The built-in identity
    // carries the selected PNG inline so local clients need no network access.
    if server_name == crate::mcp_server_identity::DEFAULT_SERVER_NAME {
        let artwork = include_bytes!("../../assets/made-cuatro-voces.png");
        result["serverInfo"]["title"] = json!("MADE");
        result["serverInfo"]["websiteUrl"] = json!("https://underpassai.com/");
        result["serverInfo"]["icons"] = json!([{
            "src": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(artwork)),
            "mimeType": "image/png",
            "sizes": ["1872x912"],
        }]);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_branding_is_offline_and_does_not_replace_host_identity() {
        let response = initialize_result(
            crate::mcp_server_identity::DEFAULT_SERVER_NAME,
            "0.6.0",
            "embedded",
            "none",
        );
        let icon = &response["serverInfo"]["icons"][0];
        let encoded = icon["src"]
            .as_str()
            .unwrap()
            .strip_prefix("data:image/png;base64,")
            .unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .unwrap(),
            include_bytes!("../../assets/made-cuatro-voces.png"),
        );
        assert_eq!(response["serverInfo"]["title"], "MADE");
        assert_eq!(
            response["serverInfo"]["websiteUrl"],
            "https://underpassai.com/"
        );
        let custom = initialize_result("host-owned", "9.1.0", "grpc", "server");
        assert_eq!(
            custom["serverInfo"],
            json!({"name": "host-owned", "version": "9.1.0"})
        );
    }
}
