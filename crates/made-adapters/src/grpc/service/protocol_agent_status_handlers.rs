use made_proto::v1 as pb;
use tonic::{Request, Response, Status};

use super::{
    domain_error_to_status, link_span_to_metadata, protocol_agent_status_from_proto,
    protocol_agent_status_page_to_proto, protocol_agent_status_query_from_proto,
    protocol_agent_status_to_proto, GrpcResult, MadeGrpcService,
};

impl MadeGrpcService {
    pub(super) async fn handle_list_protocol_agents(
        &self,
        request: Request<pb::ListProtocolAgentsRequest>,
    ) -> GrpcResult<pb::ListProtocolAgentsResponse> {
        link_span_to_metadata(&request);
        let query = protocol_agent_status_query_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let page = self
            .protocol_agent_status
            .list(query)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(protocol_agent_status_page_to_proto(&page)))
    }

    pub(super) async fn handle_get_protocol_agent(
        &self,
        request: Request<pb::GetProtocolAgentRequest>,
    ) -> GrpcResult<pb::GetProtocolAgentResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let status = self
            .protocol_agent_status
            .get(&request.protocol_id, &request.agent_execution_id)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::GetProtocolAgentResponse {
            agent: Some(protocol_agent_status_to_proto(&status)),
        }))
    }

    pub(super) async fn handle_report_protocol_agent_status(
        &self,
        request: Request<pb::ReportProtocolAgentStatusRequest>,
    ) -> GrpcResult<pb::ReportProtocolAgentStatusResponse> {
        link_span_to_metadata(&request);
        let status = protocol_agent_status_from_proto(
            request
                .into_inner()
                .status
                .ok_or_else(|| Status::invalid_argument("status is required"))?,
        )
        .map_err(domain_error_to_status)?;
        let status = self
            .protocol_agent_status
            .report(status)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::ReportProtocolAgentStatusResponse {
            status: Some(protocol_agent_status_to_proto(&status)),
        }))
    }
}
