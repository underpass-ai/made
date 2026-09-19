use made_proto::v1 as pb;
use tonic::{Request, Response, Status};

use super::{
    ceremony_agent_status_from_proto, ceremony_agent_status_page_to_proto,
    ceremony_agent_status_query_from_proto, ceremony_agent_status_to_proto, domain_error_to_status,
    link_span_to_metadata, GrpcResult, MadeGrpcService,
};

impl MadeGrpcService {
    pub(super) async fn handle_list_ceremony_agents(
        &self,
        request: Request<pb::ListCeremonyAgentsRequest>,
    ) -> GrpcResult<pb::ListCeremonyAgentsResponse> {
        link_span_to_metadata(&request);
        let query = ceremony_agent_status_query_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let page = self
            .ceremony_agent_status
            .list(query)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(ceremony_agent_status_page_to_proto(&page)))
    }

    pub(super) async fn handle_get_ceremony_agent(
        &self,
        request: Request<pb::GetCeremonyAgentRequest>,
    ) -> GrpcResult<pb::GetCeremonyAgentResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let status = self
            .ceremony_agent_status
            .get(&request.ceremony_id, &request.agent_execution_id)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::GetCeremonyAgentResponse {
            agent: Some(ceremony_agent_status_to_proto(&status)),
        }))
    }

    pub(super) async fn handle_report_ceremony_agent_status(
        &self,
        request: Request<pb::ReportCeremonyAgentStatusRequest>,
    ) -> GrpcResult<pb::ReportCeremonyAgentStatusResponse> {
        link_span_to_metadata(&request);
        let status = ceremony_agent_status_from_proto(
            request
                .into_inner()
                .status
                .ok_or_else(|| Status::invalid_argument("status is required"))?,
        )
        .map_err(domain_error_to_status)?;
        let status = self
            .ceremony_agent_status
            .report(status)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::ReportCeremonyAgentStatusResponse {
            status: Some(ceremony_agent_status_to_proto(&status)),
        }))
    }
}
