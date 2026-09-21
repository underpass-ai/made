//! The five loop RPCs, each one thin over its use case.
//!
//! Every one of them refuses honestly when no loop was composed: a
//! service built without the projection would otherwise answer an
//! empty queue and look healthy doing it.

use made_proto::v1 as pb;
use tonic::{Request, Response, Status};

use super::super::mappers::{
    acknowledge_integrator_attention_input_from_proto, attention_batch_to_proto,
    attention_delivery_page_to_proto, await_integrator_attention_input_from_proto,
    bind_ceremony_integrator_input_from_proto, bind_outcome_to_proto,
    integrator_acknowledged_to_proto, integrator_binding_state, integrator_scope_from_proto,
    list_attention_deliveries_input_from_proto,
};
use super::super::status::domain_error_to_status;
use super::integrator_loop_operations::IntegratorLoopOperations;
use super::{link_span_to_metadata, GrpcResult, MadeGrpcService};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.bind_ceremony_integrator", skip_all)]
    pub(super) async fn handle_bind_ceremony_integrator(
        &self,
        request: Request<pb::BindCeremonyIntegratorRequest>,
    ) -> GrpcResult<pb::BindCeremonyIntegratorResponse> {
        link_span_to_metadata(&request);
        let loop_operations = self.integrator_loop()?;
        let input = bind_ceremony_integrator_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let outcome = loop_operations
            .bind()
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(bind_outcome_to_proto(&outcome)))
    }

    #[tracing::instrument(name = "rpc.get_ceremony_integrator_binding", skip_all)]
    pub(super) async fn handle_get_ceremony_integrator_binding(
        &self,
        request: Request<pb::GetCeremonyIntegratorBindingRequest>,
    ) -> GrpcResult<pb::GetCeremonyIntegratorBindingResponse> {
        link_span_to_metadata(&request);
        let loop_operations = self.integrator_loop()?;
        let scope = integrator_scope_from_proto(request.into_inner().scope)
            .map_err(domain_error_to_status)?;
        let binding = loop_operations
            .binding()
            .execute(&scope)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::GetCeremonyIntegratorBindingResponse {
            binding: binding.as_ref().map(integrator_binding_state),
        }))
    }

    #[tracing::instrument(name = "rpc.await_integrator_attention", skip_all)]
    pub(super) async fn handle_await_integrator_attention(
        &self,
        request: Request<pb::AwaitIntegratorAttentionRequest>,
    ) -> GrpcResult<pb::AwaitIntegratorAttentionResponse> {
        link_span_to_metadata(&request);
        let loop_operations = self.integrator_loop()?;
        let input = await_integrator_attention_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let batch = loop_operations
            .await_attention()
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(attention_batch_to_proto(&batch)))
    }

    #[tracing::instrument(name = "rpc.acknowledge_integrator_attention", skip_all)]
    pub(super) async fn handle_acknowledge_integrator_attention(
        &self,
        request: Request<pb::AcknowledgeIntegratorAttentionRequest>,
    ) -> GrpcResult<pb::AcknowledgeIntegratorAttentionResponse> {
        link_span_to_metadata(&request);
        let loop_operations = self.integrator_loop()?;
        let input = acknowledge_integrator_attention_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let acknowledged = loop_operations
            .acknowledge()
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(integrator_acknowledged_to_proto(
            &acknowledged,
        )))
    }

    #[tracing::instrument(name = "rpc.list_attention_deliveries", skip_all)]
    pub(super) async fn handle_list_attention_deliveries(
        &self,
        request: Request<pb::ListAttentionDeliveriesRequest>,
    ) -> GrpcResult<pb::ListAttentionDeliveriesResponse> {
        link_span_to_metadata(&request);
        let loop_operations = self.integrator_loop()?;
        let input = list_attention_deliveries_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let page = loop_operations
            .deliveries()
            .execute(input)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(attention_delivery_page_to_proto(&page)))
    }

    /// The composed loop, or an honest refusal.
    fn integrator_loop(&self) -> Result<&IntegratorLoopOperations, Status> {
        self.integrator_loop.as_deref().ok_or_else(|| {
            Status::unimplemented(
                "this composition has no integrator loop; nothing is bound and nothing is delivered",
            )
        })
    }
}
