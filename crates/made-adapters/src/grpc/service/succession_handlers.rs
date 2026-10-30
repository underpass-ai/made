use super::{domain_error_to_status, pb, GrpcResult, MadeGrpcService, Request, Response, Status};
use crate::grpc::mappers::protocol_succession::{
    plan_input, plan_to_proto, plan_view_to_proto, start_input,
};

impl MadeGrpcService {
    pub(super) async fn handle_plan_protocol_successor(
        &self,
        request: Request<pb::PlanProtocolSuccessorRequest>,
    ) -> GrpcResult<pb::PlanProtocolSuccessorResponse> {
        let input = plan_input(request.into_inner()).map_err(domain_error_to_status)?;
        let view = self
            .plan_protocol_successor
            .as_ref()
            .ok_or_else(|| Status::unimplemented("protocol succession not configured"))?
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::PlanProtocolSuccessorResponse {
            plan: Some(plan_view_to_proto(view)),
        }))
    }

    pub(super) async fn handle_start_protocol_successor(
        &self,
        request: Request<pb::StartProtocolSuccessorRequest>,
    ) -> GrpcResult<pb::StartProtocolSuccessorResponse> {
        let input = start_input(request.into_inner()).map_err(domain_error_to_status)?;
        let outcome = self
            .start_protocol_successor
            .as_ref()
            .ok_or_else(|| Status::unimplemented("protocol succession not configured"))?
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::StartProtocolSuccessorResponse {
            successor_id: outcome.successor.id().as_str().to_owned(),
            plan: Some(plan_to_proto(&outcome.plan)),
        }))
    }
}
