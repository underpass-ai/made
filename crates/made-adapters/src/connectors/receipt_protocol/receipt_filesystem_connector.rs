use super::SafeExecutionReceipt;
use crate::connectors::{ConnectorDescriptor, ConnectorError};
use async_trait::async_trait;
use made_core::value_objects::ExecutionOperationId;

#[async_trait]
pub trait ReceiptFilesystemConnector: Send + Sync {
    fn descriptor(&self) -> &ConnectorDescriptor;
    async fn load(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Option<SafeExecutionReceipt>, ConnectorError>;
    async fn store(&self, receipt: &SafeExecutionReceipt) -> Result<(), ConnectorError>;
}
