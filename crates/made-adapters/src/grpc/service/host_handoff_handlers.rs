use super::{domain_error_to_status, pb, GrpcResult, MadeGrpcService, Request, Response, Status};
use crate::grpc::mappers::{
    host_handoff::{handoff_input, handoff_to_proto, preflight_input},
    protocol_preflight::preflight_to_proto,
};

impl MadeGrpcService {
    pub(super) async fn handle_record_protocol_host_handoff(
        &self,
        request: Request<pb::RecordProtocolHostHandoffRequest>,
    ) -> GrpcResult<pb::RecordProtocolHostHandoffResponse> {
        let input = handoff_input(request.into_inner()).map_err(domain_error_to_status)?;
        let recorded = self
            .record_protocol_host_handoff
            .as_ref()
            .ok_or_else(|| Status::unimplemented("host handoff not configured"))?
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::RecordProtocolHostHandoffResponse {
            recorded: Some(handoff_to_proto(recorded)),
        }))
    }

    pub(super) async fn handle_inspect_protocol_resume(
        &self,
        request: Request<pb::InspectProtocolResumeRequest>,
    ) -> GrpcResult<pb::InspectProtocolResumeResponse> {
        let input = preflight_input(request.into_inner()).map_err(domain_error_to_status)?;
        let report = self
            .inspect_protocol_resume
            .as_ref()
            .ok_or_else(|| Status::unimplemented("resume preflight not configured"))?
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::InspectProtocolResumeResponse {
            report: Some(preflight_to_proto(report)),
        }))
    }
}
