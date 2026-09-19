use tonic::metadata::MetadataValue;
use tonic::service::Interceptor;
use tonic::{Request, Status};
use uuid::Uuid;

/// Gives every remote E2E invocation an explicit authorization identity.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct E2eRequestIdInterceptor;

impl Interceptor for E2eRequestIdInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let request_id = Uuid::new_v4().to_string();
        let value =
            MetadataValue::try_from(request_id).expect("UUID request id is valid gRPC metadata");
        request.metadata_mut().insert("x-made-request-id", value);
        Ok(request)
    }
}
