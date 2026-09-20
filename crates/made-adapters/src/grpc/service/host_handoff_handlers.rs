use super::{domain_error_to_status, pb, GrpcResult, MadeGrpcService, Request, Response, Status};
use crate::grpc::mappers::{
    ceremony_preflight::preflight_to_proto,
    host_handoff::{handoff_input, handoff_to_proto, preflight_input},
};

impl MadeGrpcService {
    pub(super) async fn handle_record_ceremony_host_handoff(
        &self,
        request: Request<pb::RecordCeremonyHostHandoffRequest>,
    ) -> GrpcResult<pb::RecordCeremonyHostHandoffResponse> {
        let input = handoff_input(request.into_inner()).map_err(domain_error_to_status)?;
        let recorded = self
            .record_ceremony_host_handoff
            .as_ref()
            .ok_or_else(|| Status::unimplemented("host handoff not configured"))?
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::RecordCeremonyHostHandoffResponse {
            recorded: Some(handoff_to_proto(recorded)),
        }))
    }

    pub(super) async fn handle_inspect_ceremony_resume(
        &self,
        request: Request<pb::InspectCeremonyResumeRequest>,
    ) -> GrpcResult<pb::InspectCeremonyResumeResponse> {
        let input = preflight_input(request.into_inner()).map_err(domain_error_to_status)?;
        let report = self
            .inspect_ceremony_resume
            .as_ref()
            .ok_or_else(|| Status::unimplemented("resume preflight not configured"))?
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::InspectCeremonyResumeResponse {
            report: Some(preflight_to_proto(report)),
        }))
    }
}
