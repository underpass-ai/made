use made_proto::v1::{
    ExecutionReceipt, GetExecutionReceiptRequest, InspectExecutionRecoveryRequest,
    InspectExecutionRecoveryResponse,
};

use crate::{MadeClient, MadeClientError};

impl MadeClient {
    pub async fn get_execution_receipt(
        &self,
        operation_id: impl Into<String>,
    ) -> Result<ExecutionReceipt, MadeClientError> {
        let response = self
            .rpc()
            .get_execution_receipt(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/GetExecutionReceipt",
                GetExecutionReceiptRequest {
                    operation_id: operation_id.into(),
                },
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        response.receipt.ok_or_else(|| {
            MadeClientError::ProtocolViolation(
                "get execution receipt response has no receipt".to_owned(),
            )
        })
    }

    pub async fn inspect_execution_recovery(
        &self,
        after: Option<String>,
        limit: u32,
    ) -> Result<InspectExecutionRecoveryResponse, MadeClientError> {
        self.rpc()
            .inspect_execution_recovery(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/InspectExecutionRecovery",
                InspectExecutionRecoveryRequest { after, limit },
            ))
            .await
            .map(tonic::Response::into_inner)
            .map_err(MadeClientError::from_status)
    }
}
