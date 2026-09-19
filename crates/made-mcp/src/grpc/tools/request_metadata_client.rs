use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use tonic::metadata::MetadataValue;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;

use crate::protocol::ToolError;

pub(super) fn build(
    channel: Channel,
    traceparent: &str,
    request_id: &str,
    target_digest: &str,
) -> Result<MadeServiceClient<InterceptedService<Channel, RequestMetadataInterceptor>>, ToolError> {
    let traceparent = MetadataValue::try_from(traceparent)
        .map_err(|error| ToolError::invalid_request(error.to_string()))?;
    let request_id = MetadataValue::try_from(request_id)
        .map_err(|error| ToolError::invalid_request(error.to_string()))?;
    let target_digest = MetadataValue::try_from(target_digest)
        .map_err(|error| ToolError::invalid_request(error.to_string()))?;
    Ok(MadeServiceClient::with_interceptor(
        channel,
        RequestMetadataInterceptor {
            traceparent,
            request_id,
            target_digest,
        },
    ))
}

#[derive(Clone)]
pub(super) struct RequestMetadataInterceptor {
    traceparent: MetadataValue<tonic::metadata::Ascii>,
    request_id: MetadataValue<tonic::metadata::Ascii>,
    target_digest: MetadataValue<tonic::metadata::Ascii>,
}

impl tonic::service::Interceptor for RequestMetadataInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        request
            .metadata_mut()
            .insert("traceparent", self.traceparent.clone());
        request
            .metadata_mut()
            .insert("x-made-request-id", self.request_id.clone());
        request
            .metadata_mut()
            .insert("x-made-target-digest", self.target_digest.clone());
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use tonic::service::Interceptor;

    use super::*;

    #[test]
    fn proxy_metadata_carries_request_and_canonical_target_identities() {
        let mut interceptor = RequestMetadataInterceptor {
            traceparent: "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
                .parse()
                .unwrap(),
            request_id: "stable-retry".parse().unwrap(),
            target_digest: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .parse()
                .unwrap(),
        };

        let request = interceptor.call(tonic::Request::new(())).unwrap();

        assert_eq!(
            request
                .metadata()
                .get("x-made-request-id")
                .unwrap()
                .to_str()
                .unwrap(),
            "stable-retry"
        );
        assert_eq!(
            request
                .metadata()
                .get("x-made-target-digest")
                .unwrap()
                .to_str()
                .unwrap(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    }
}
