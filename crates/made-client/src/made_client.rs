use std::cmp::min;

use made_proto::v1::made_service_client::MadeServiceClient;
use tokio::time::sleep;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};
use tonic::Request;

use crate::{ClientConfig, MadeClientError};

/// Cloneable reference client backed exclusively by MADE's public gRPC API.
#[derive(Clone, Debug)]
pub struct MadeClient {
    channel: Channel,
    request_id: MetadataValue<Ascii>,
}

impl MadeClient {
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, MadeClientError> {
        Self::connect_with_config(ClientConfig::new(endpoint)).await
    }

    pub async fn connect_with_config(config: ClientConfig) -> Result<Self, MadeClientError> {
        let request_id = MetadataValue::try_from(config.request_id())
            .map_err(|error| MadeClientError::InvalidRequestId(error.to_string()))?;
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
                        request_id,
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

    pub(crate) fn request<T>(&self, payload: T) -> Request<T> {
        let mut request = Request::new(payload);
        request
            .metadata_mut()
            .insert("x-made-request-id", self.request_id.clone());
        request
    }

    #[must_use]
    pub fn request_id(&self) -> &str {
        self.request_id.to_str().expect("ASCII metadata is UTF-8")
    }
}
