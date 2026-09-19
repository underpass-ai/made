use std::cmp::min;

use made_proto::v1::made_service_client::MadeServiceClient;
use prost::Message;
use sha2::{Digest, Sha256};
use tokio::time::sleep;
use tonic::metadata::MetadataValue;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};
use tonic::Request;

use crate::{ClientConfig, MadeClientError};

/// Cloneable reference client backed exclusively by MADE's public gRPC API.
#[derive(Clone, Debug)]
pub struct MadeClient {
    channel: Channel,
    invocation_id: String,
}

impl MadeClient {
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, MadeClientError> {
        Self::connect_with_config(ClientConfig::new(endpoint)).await
    }

    pub async fn connect_with_config(config: ClientConfig) -> Result<Self, MadeClientError> {
        let invocation_id = config.invocation_id().trim();
        if invocation_id.is_empty() || invocation_id.len() > 256 {
            return Err(MadeClientError::InvalidInvocationId);
        }
        let mut endpoint = Endpoint::from_shared(config.endpoint().to_owned())
            .map_err(|error| MadeClientError::InvalidEndpoint(error.to_string()))?;
        if config.uses_tls() {
            let mut tls = ClientTlsConfig::new().with_native_roots();
            if let Some(pem) = config.ca_certificate_pem() {
                tls = tls.ca_certificate(Certificate::from_pem(pem));
            }
            if let Some((certificate, key)) = config.client_identity_pem() {
                tls = tls.identity(Identity::from_pem(certificate, key));
            }
            if let Some(domain_name) = config.tls_domain_name() {
                tls = tls.domain_name(domain_name);
            }
            endpoint = endpoint
                .tls_config(tls)
                .map_err(|error| MadeClientError::TlsConfiguration(error.to_string()))?;
        }
        let mut delay = config.initial_backoff();
        let mut last_error = None;

        for attempt in 0..config.connect_attempts() {
            match endpoint.clone().connect().await {
                Ok(channel) => {
                    return Ok(Self {
                        channel,
                        invocation_id: invocation_id.to_owned(),
                    });
                }
                Err(error) => last_error = Some(error.to_string()),
            }
            if attempt + 1 < config.connect_attempts() {
                sleep(delay).await;
                delay = min(delay.saturating_mul(2), config.maximum_backoff());
            }
        }

        Err(MadeClientError::ConnectionExhausted(
            last_error.unwrap_or_else(|| "no connection attempt was made".to_owned()),
        ))
    }

    pub(crate) fn rpc(&self) -> MadeServiceClient<Channel> {
        MadeServiceClient::new(self.channel.clone())
    }

    pub(crate) fn request<T>(&self, method: &'static str, payload: T) -> Request<T>
    where
        T: Message,
    {
        let payload_bytes = payload.encode_to_vec();
        let request_id = derived_request_id(&self.invocation_id, method, &payload_bytes);
        let mut request = Request::new(payload);
        request
            .metadata_mut()
            .insert("x-made-request-id", request_id);
        request
    }

    #[must_use]
    pub fn invocation_id(&self) -> &str {
        &self.invocation_id
    }
}

fn derived_request_id(
    invocation_id: &str,
    method: &'static str,
    payload: &[u8],
) -> MetadataValue<tonic::metadata::Ascii> {
    let mut digest = Sha256::new();
    digest.update(b"underpass.made.client-request.v1\0");
    hash_field(&mut digest, invocation_id.as_bytes());
    hash_field(&mut digest, method.as_bytes());
    hash_field(&mut digest, payload);
    let value = format!("made-{:x}", digest.finalize());
    MetadataValue::try_from(value).expect("SHA-256 request ids are valid ASCII metadata")
}

fn hash_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}
