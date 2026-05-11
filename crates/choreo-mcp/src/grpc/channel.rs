//! Tonic channel construction + TLS configuration.

use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};

use crate::backend::{ChoreoMcpGrpcTlsConfig, ChoreoMcpGrpcTlsMode};

/// Open a tonic channel honouring the configured TLS posture.
///
/// Mistakes surface as plain strings so the MCP tool result can carry
/// them straight to the agent without leaking tonic internals.
pub(crate) async fn open_channel(
    endpoint: &str,
    tls: &ChoreoMcpGrpcTlsConfig,
) -> Result<Channel, String> {
    let mut endpoint_builder = Endpoint::from_shared(endpoint.to_string())
        .map_err(|err| format!("invalid gRPC endpoint `{endpoint}`: {err}"))?;

    if tls.mode() != ChoreoMcpGrpcTlsMode::Disabled {
        let tls_config = build_client_tls_config(tls).await?;
        endpoint_builder = endpoint_builder
            .tls_config(tls_config)
            .map_err(|err| format!("failed to apply client TLS config: {err}"))?;
    }

    endpoint_builder
        .connect()
        .await
        .map_err(|err| format!("gRPC connect to {endpoint} failed: {err}"))
}

async fn build_client_tls_config(tls: &ChoreoMcpGrpcTlsConfig) -> Result<ClientTlsConfig, String> {
    let mut config = ClientTlsConfig::new();

    if let Some(domain) = tls.domain_name.as_deref() {
        config = config.domain_name(domain.to_string());
    }

    if let Some(ca_path) = tls.ca_path.as_ref() {
        let pem = tokio::fs::read(ca_path)
            .await
            .map_err(|err| format!("failed to read TLS CA at {}: {err}", ca_path.display()))?;
        config = config.ca_certificate(Certificate::from_pem(pem));
    }

    if tls.mode() == ChoreoMcpGrpcTlsMode::Mutual {
        let cert_path = tls
            .cert_path
            .as_ref()
            .ok_or_else(|| "TLS mode=mutual requires CHOREO_MCP_GRPC_TLS_CERT_PATH".to_string())?;
        let key_path = tls
            .key_path
            .as_ref()
            .ok_or_else(|| "TLS mode=mutual requires CHOREO_MCP_GRPC_TLS_KEY_PATH".to_string())?;
        let cert = tokio::fs::read(cert_path).await.map_err(|err| {
            format!(
                "failed to read client cert at {}: {err}",
                cert_path.display()
            )
        })?;
        let key = tokio::fs::read(key_path)
            .await
            .map_err(|err| format!("failed to read client key at {}: {err}", key_path.display()))?;
        config = config.identity(Identity::from_pem(cert, key));
    }

    Ok(config)
}
