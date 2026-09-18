use super::{
    cancel_ceremony_input_from_proto, domain_error_to_status,
    enforce_ceremony_deadlines_input_from_proto, link_span_to_metadata,
    pause_ceremony_input_from_proto, pb, resume_ceremony_input_from_proto, GrpcResult,
    MadeGrpcService, Request, Response,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.pause_ceremony", skip_all)]
    pub(super) async fn handle_pause_ceremony(
        &self,
        request: Request<pb::PauseCeremonyRequest>,
    ) -> GrpcResult<pb::PauseCeremonyResponse> {
        link_span_to_metadata(&request);
        let input = pause_ceremony_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .pause_ceremony
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::PauseCeremonyResponse {
            instance: Some(self.project(&instance).await?),
        }))
    }

    #[tracing::instrument(name = "rpc.resume_ceremony", skip_all)]
    pub(super) async fn handle_resume_ceremony(
        &self,
        request: Request<pb::ResumeCeremonyRequest>,
    ) -> GrpcResult<pb::ResumeCeremonyResponse> {
        link_span_to_metadata(&request);
        let input = resume_ceremony_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .resume_ceremony
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::ResumeCeremonyResponse {
            instance: Some(self.project(&instance).await?),
        }))
    }

    #[tracing::instrument(name = "rpc.cancel_ceremony", skip_all)]
    pub(super) async fn handle_cancel_ceremony(
        &self,
        request: Request<pb::CancelCeremonyRequest>,
    ) -> GrpcResult<pb::CancelCeremonyResponse> {
        link_span_to_metadata(&request);
        let input = cancel_ceremony_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .cancel_ceremony
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::CancelCeremonyResponse {
            instance: Some(self.project(&instance).await?),
        }))
    }

    #[tracing::instrument(name = "rpc.enforce_ceremony_deadlines", skip_all)]
    pub(super) async fn handle_enforce_ceremony_deadlines(
        &self,
        request: Request<pb::EnforceCeremonyDeadlinesRequest>,
    ) -> GrpcResult<pb::EnforceCeremonyDeadlinesResponse> {
        link_span_to_metadata(&request);
        let input = enforce_ceremony_deadlines_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .enforce_ceremony_deadlines
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::EnforceCeremonyDeadlinesResponse {
            instance: Some(self.project(&instance).await?),
        }))
    }
}
