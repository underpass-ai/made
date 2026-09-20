use super::{domain_error_to_status, pb, GrpcResult, MadeGrpcService, Request, Response, Status};
use crate::grpc::mappers::ceremony_instance_state_from;
use crate::grpc::mappers::ceremony_succession::{
    plan_input, plan_to_proto, plan_view_to_proto, start_input,
};
use made_app::usecases::CeremonyInstanceView;

impl MadeGrpcService {
    pub(super) async fn handle_plan_ceremony_successor(
        &self,
        request: Request<pb::PlanCeremonySuccessorRequest>,
    ) -> GrpcResult<pb::PlanCeremonySuccessorResponse> {
        let input = plan_input(request.into_inner()).map_err(domain_error_to_status)?;
        let view = self
            .plan_ceremony_successor
            .as_ref()
            .ok_or_else(|| Status::unimplemented("ceremony succession not configured"))?
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::PlanCeremonySuccessorResponse {
            plan: Some(plan_view_to_proto(view)),
        }))
    }

    pub(super) async fn handle_start_ceremony_successor(
        &self,
        request: Request<pb::StartCeremonySuccessorRequest>,
    ) -> GrpcResult<pb::StartCeremonySuccessorResponse> {
        let input = start_input(request.into_inner()).map_err(domain_error_to_status)?;
        let outcome = self
            .start_ceremony_successor
            .as_ref()
            .ok_or_else(|| Status::unimplemented("ceremony succession not configured"))?
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let definition = self
            .resolve_ceremony_definition
            .execute(&outcome.successor)
            .await
            .map_err(domain_error_to_status)?;
        let view = CeremonyInstanceView::project(&outcome.successor, &definition)
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::StartCeremonySuccessorResponse {
            successor: Some(ceremony_instance_state_from(&view)),
            plan: Some(plan_to_proto(&outcome.plan)),
        }))
    }
}
