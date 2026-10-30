use super::{
    cancel_protocol_input_from_proto, domain_error_to_status,
    enforce_protocol_deadlines_input_from_proto, link_span_to_metadata,
    pause_protocol_input_from_proto, pb, resume_protocol_input_from_proto, GrpcResult,
    MadeGrpcService, Request, Response,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.pause_protocol", skip_all)]
    pub(super) async fn handle_pause_protocol(
        &self,
        request: Request<pb::PauseProtocolRequest>,
    ) -> GrpcResult<pb::PauseProtocolResponse> {
        link_span_to_metadata(&request);
        let input = pause_protocol_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .pause_protocol
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::PauseProtocolResponse {
            instance: Some(self.project(&instance).await?),
        }))
    }

    #[tracing::instrument(name = "rpc.resume_protocol", skip_all)]
    pub(super) async fn handle_resume_protocol(
        &self,
        request: Request<pb::ResumeProtocolRequest>,
    ) -> GrpcResult<pb::ResumeProtocolResponse> {
        link_span_to_metadata(&request);
        let input = resume_protocol_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .resume_protocol
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::ResumeProtocolResponse {
            instance: Some(self.project(&instance).await?),
        }))
    }

    #[tracing::instrument(name = "rpc.cancel_protocol", skip_all)]
    pub(super) async fn handle_cancel_protocol(
        &self,
        request: Request<pb::CancelProtocolRequest>,
    ) -> GrpcResult<pb::CancelProtocolResponse> {
        link_span_to_metadata(&request);
        let input = cancel_protocol_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .cancel_protocol
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::CancelProtocolResponse {
            instance: Some(self.project(&instance).await?),
        }))
    }

    #[tracing::instrument(name = "rpc.enforce_protocol_deadlines", skip_all)]
    pub(super) async fn handle_enforce_protocol_deadlines(
        &self,
        request: Request<pb::EnforceProtocolDeadlinesRequest>,
    ) -> GrpcResult<pb::EnforceProtocolDeadlinesResponse> {
        link_span_to_metadata(&request);
        let input = enforce_protocol_deadlines_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .enforce_protocol_deadlines
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::EnforceProtocolDeadlinesResponse {
            instance: Some(self.project(&instance).await?),
        }))
    }
}
