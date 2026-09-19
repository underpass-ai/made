//! Public inspection and fenced application of durable execution receipts.

use made_core::value_objects::ExecutionOperationId;

use super::{
    adopt_execution_receipt_input_from_proto, complete_execution_receipt_input_from_proto,
    domain_error_to_status, execution_receipt_to_proto, execution_recovery_cursor_from_proto,
    execution_recovery_limit_from_proto, execution_recovery_page_to_proto, link_span_to_metadata,
    pb, GrpcResult, MadeGrpcService, Request, Response,
};

impl MadeGrpcService {
    #[tracing::instrument(name = "rpc.get_execution_receipt", skip_all)]
    pub(super) async fn handle_get_execution_receipt(
        &self,
        request: Request<pb::GetExecutionReceiptRequest>,
    ) -> GrpcResult<pb::GetExecutionReceiptResponse> {
        link_span_to_metadata(&request);
        let operation_id = ExecutionOperationId::new(request.into_inner().operation_id)
            .map_err(domain_error_to_status)?;
        let receipt = self
            .get_execution_receipt
            .as_ref()
            .ok_or_else(receipts_unconfigured)?
            .execute(&operation_id)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(pb::GetExecutionReceiptResponse {
            receipt: Some(execution_receipt_to_proto(&receipt)),
        }))
    }

    #[tracing::instrument(name = "rpc.inspect_execution_recovery", skip_all)]
    pub(super) async fn handle_inspect_execution_recovery(
        &self,
        request: Request<pb::InspectExecutionRecoveryRequest>,
    ) -> GrpcResult<pb::InspectExecutionRecoveryResponse> {
        link_span_to_metadata(&request);
        let request = request.into_inner();
        let cursor =
            execution_recovery_cursor_from_proto(request.after).map_err(domain_error_to_status)?;
        let limit =
            execution_recovery_limit_from_proto(request.limit).map_err(domain_error_to_status)?;
        let page = self
            .inspect_execution_recovery
            .as_ref()
            .ok_or_else(receipts_unconfigured)?
            .execute(cursor.as_ref(), limit)
            .await
            .map_err(domain_error_to_status)?;
        Ok(Response::new(execution_recovery_page_to_proto(&page)))
    }

    #[tracing::instrument(name = "rpc.complete_execution_receipt", skip_all)]
    pub(super) async fn handle_complete_execution_receipt(
        &self,
        request: Request<pb::CompleteExecutionReceiptRequest>,
    ) -> GrpcResult<pb::CompleteExecutionReceiptResponse> {
        link_span_to_metadata(&request);
        let input = complete_execution_receipt_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self.apply_execution_receipt(input).await?;
        Ok(Response::new(pb::CompleteExecutionReceiptResponse {
            instance: Some(self.project(&instance).await?),
        }))
    }

    #[tracing::instrument(name = "rpc.adopt_execution_receipt", skip_all)]
    pub(super) async fn handle_adopt_execution_receipt(
        &self,
        request: Request<pb::AdoptExecutionReceiptRequest>,
    ) -> GrpcResult<pb::AdoptExecutionReceiptResponse> {
        link_span_to_metadata(&request);
        let input = adopt_execution_receipt_input_from_proto(request.into_inner())
            .map_err(domain_error_to_status)?;
        let instance = self.apply_execution_receipt(input).await?;
        Ok(Response::new(pb::AdoptExecutionReceiptResponse {
            instance: Some(self.project(&instance).await?),
        }))
    }

    async fn apply_execution_receipt(
        &self,
        input: made_app::workers::CompleteExecutionReceiptInput,
    ) -> Result<made_core::entities::CeremonyInstance, tonic::Status> {
        self.complete_execution_receipt
            .as_ref()
            .ok_or_else(receipts_unconfigured)?
            .execute(input)
            .await
            .map_err(domain_error_to_status)
    }
}

fn receipts_unconfigured() -> tonic::Status {
    domain_error_to_status(made_core::error::DomainError::InvariantViolated {
        reason: "execution receipt services are not configured",
    })
}
