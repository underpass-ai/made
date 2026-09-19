use std::cmp::min;

use made_proto::v1::made_service_client::MadeServiceClient;
use tokio::time::sleep;
use tonic::transport::{Channel, Endpoint};

use crate::{ClientConfig, MadeClientError};

/// Cloneable reference client backed exclusively by MADE's public gRPC API.
#[derive(Clone, Debug)]
pub struct MadeClient {
    channel: Channel,
}

impl MadeClient {
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, MadeClientError> {
        Self::connect_with_config(ClientConfig::new(endpoint)).await
    }

    pub async fn connect_with_config(config: ClientConfig) -> Result<Self, MadeClientError> {
        let endpoint = Endpoint::from_shared(config.endpoint().to_owned())
            .map_err(|error| MadeClientError::InvalidEndpoint(error.to_string()))?;
        let mut delay = config.initial_backoff();
        let mut last_error = None;

        for attempt in 0..config.connect_attempts() {
            match endpoint.clone().connect().await {
                Ok(channel) => return Ok(Self { channel }),
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
}
