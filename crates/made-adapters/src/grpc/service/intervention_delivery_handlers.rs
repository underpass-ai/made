//! The four delivery RPCs, each one thin over its use case.

use made_proto::v1 as pb;
use tonic::{Request, Response};

use super::super::mappers::{
    acknowledge_protocol_agent_intervention_input_from_proto,
    get_protocol_intervention_input_from_proto, intervention_delivery_state,
    intervention_page_to_proto, list_protocol_interventions_input_from_proto,
    pull_protocol_agent_interventions_input_from_proto, pulled_interventions_to_proto,
};
use super::super::status::domain_error_to_status;
use super::{link_span_to_metadata, GrpcResult, MadeGrpcService};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.pull_protocol_agent_interventions", skip_all)]
    pub(super) async fn handle_pull_protocol_agent_interventions(
        &self,
        request: Request<pb::PullProtocolAgentInterventionsRequest>,
    ) -> GrpcResult<pb::PullProtocolAgentInterventionsResponse> {
        link_span_to_metadata(&request);
        let input = pull_protocol_agent_interventions_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let pulled = self
            .pull_protocol_agent_interventions
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::PullProtocolAgentInterventionsResponse {
            items: pulled_interventions_to_proto(&pulled),
        }))
    }

    #[tracing::instrument(name = "rpc.acknowledge_protocol_agent_intervention", skip_all)]
    pub(super) async fn handle_acknowledge_protocol_agent_intervention(
        &self,
        request: Request<pb::AcknowledgeProtocolAgentInterventionRequest>,
    ) -> GrpcResult<pb::AcknowledgeProtocolAgentInterventionResponse> {
        link_span_to_metadata(&request);
        let input = acknowledge_protocol_agent_intervention_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .acknowledge_protocol_agent_intervention
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(
            pb::AcknowledgeProtocolAgentInterventionResponse {
                instance: Some(state),
            },
        ))
    }

    #[tracing::instrument(name = "rpc.get_protocol_intervention", skip_all)]
    pub(super) async fn handle_get_protocol_intervention(
        &self,
        request: Request<pb::GetProtocolInterventionRequest>,
    ) -> GrpcResult<pb::GetProtocolInterventionResponse> {
        link_span_to_metadata(&request);
        let input = get_protocol_intervention_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let view = self
            .get_protocol_intervention
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::GetProtocolInterventionResponse {
            intervention: Some(intervention_delivery_state(&view)),
        }))
    }

    #[tracing::instrument(name = "rpc.list_protocol_interventions", skip_all)]
    pub(super) async fn handle_list_protocol_interventions(
        &self,
        request: Request<pb::ListProtocolInterventionsRequest>,
    ) -> GrpcResult<pb::ListProtocolInterventionsResponse> {
        link_span_to_metadata(&request);
        let input = list_protocol_interventions_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let page = self
            .list_protocol_interventions
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::ListProtocolInterventionsResponse {
            interventions: intervention_page_to_proto(&page),
            next_cursor: page
                .next_cursor()
                .map(|cursor| cursor.as_str().to_owned())
                .unwrap_or_default(),
        }))
    }
}
