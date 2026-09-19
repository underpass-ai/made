use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use tonic::metadata::MetadataValue;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;

use crate::protocol::ToolError;

pub(super) fn build(
    channel: Channel,
    traceparent: &str,
    request_id: &str,
) -> Result<MadeServiceClient<InterceptedService<Channel, RequestMetadataInterceptor>>, ToolError> {
    let traceparent = MetadataValue::try_from(traceparent)
        .map_err(|error| ToolError::invalid_request(error.to_string()))?;
    let request_id = MetadataValue::try_from(request_id)
        .map_err(|error| ToolError::invalid_request(error.to_string()))?;
    Ok(MadeServiceClient::with_interceptor(
        channel,
        RequestMetadataInterceptor {
            traceparent,
            request_id,
        },
    ))
}

#[derive(Clone)]
pub(super) struct RequestMetadataInterceptor {
    traceparent: MetadataValue<tonic::metadata::Ascii>,
    request_id: MetadataValue<tonic::metadata::Ascii>,
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
        Ok(request)
    }
}
