//! The four delivery RPCs, each one thin over its use case.

use made_proto::v1 as pb;
use tonic::{Request, Response};

use super::super::mappers::{
    acknowledge_ceremony_agent_intervention_input_from_proto,
    get_ceremony_intervention_input_from_proto, intervention_delivery_state,
    intervention_page_to_proto, list_ceremony_interventions_input_from_proto,
    pull_ceremony_agent_interventions_input_from_proto, pulled_interventions_to_proto,
};
use super::super::status::domain_error_to_status;
use super::{link_span_to_metadata, GrpcResult, MadeGrpcService};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.pull_ceremony_agent_interventions", skip_all)]
    pub(super) async fn handle_pull_ceremony_agent_interventions(
        &self,
        request: Request<pb::PullCeremonyAgentInterventionsRequest>,
    ) -> GrpcResult<pb::PullCeremonyAgentInterventionsResponse> {
        link_span_to_metadata(&request);
        let input = pull_ceremony_agent_interventions_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let pulled = self
            .pull_ceremony_agent_interventions
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::PullCeremonyAgentInterventionsResponse {
            items: pulled_interventions_to_proto(&pulled),
        }))
    }

    #[tracing::instrument(name = "rpc.acknowledge_ceremony_agent_intervention", skip_all)]
    pub(super) async fn handle_acknowledge_ceremony_agent_intervention(
        &self,
        request: Request<pb::AcknowledgeCeremonyAgentInterventionRequest>,
    ) -> GrpcResult<pb::AcknowledgeCeremonyAgentInterventionResponse> {
        link_span_to_metadata(&request);
        let input = acknowledge_ceremony_agent_intervention_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self
            .acknowledge_ceremony_agent_intervention
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        let state = self.project(&instance).await?;
        Ok(Response::new(
            pb::AcknowledgeCeremonyAgentInterventionResponse {
                instance: Some(state),
            },
        ))
    }

    #[tracing::instrument(name = "rpc.get_ceremony_intervention", skip_all)]
    pub(super) async fn handle_get_ceremony_intervention(
        &self,
        request: Request<pb::GetCeremonyInterventionRequest>,
    ) -> GrpcResult<pb::GetCeremonyInterventionResponse> {
        link_span_to_metadata(&request);
        let input = get_ceremony_intervention_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let view = self
            .get_ceremony_intervention
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::GetCeremonyInterventionResponse {
            intervention: Some(intervention_delivery_state(&view)),
        }))
    }

    #[tracing::instrument(name = "rpc.list_ceremony_interventions", skip_all)]
    pub(super) async fn handle_list_ceremony_interventions(
        &self,
        request: Request<pb::ListCeremonyInterventionsRequest>,
    ) -> GrpcResult<pb::ListCeremonyInterventionsResponse> {
        link_span_to_metadata(&request);
        let input = list_ceremony_interventions_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let page = self
            .list_ceremony_interventions
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::ListCeremonyInterventionsResponse {
            interventions: intervention_page_to_proto(&page),
            next_cursor: page
                .next_cursor()
                .map(|cursor| cursor.as_str().to_owned())
                .unwrap_or_default(),
        }))
    }
}
