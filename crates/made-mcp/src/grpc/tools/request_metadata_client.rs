use made_mcp_proto::v1::made_service_client::MadeServiceClient;
use tonic::metadata::MetadataValue;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;

use crate::protocol::ToolError;

#[allow(clippy::result_large_err)] // tonic interceptor requires a Status error.
pub(super) fn build(
    channel: Channel,
    traceparent: &str,
    request_id: &str,
) -> Result<
    MadeServiceClient<
        InterceptedService<
            Channel,
            impl tonic::service::Interceptor + Clone + Send + Sync + 'static,
        >,
    >,
    ToolError,
> {
    let traceparent = MetadataValue::try_from(traceparent)
        .map_err(|error| ToolError::invalid_request(error.to_string()))?;
    let request_id = MetadataValue::try_from(request_id)
        .map_err(|error| ToolError::invalid_request(error.to_string()))?;
    Ok(MadeServiceClient::with_interceptor(
        channel,
        move |mut request: tonic::Request<()>| {
            request
                .metadata_mut()
                .insert("traceparent", traceparent.clone());
            request
                .metadata_mut()
                .insert("x-made-request-id", request_id.clone());
            Ok(request)
        },
    ))
}
