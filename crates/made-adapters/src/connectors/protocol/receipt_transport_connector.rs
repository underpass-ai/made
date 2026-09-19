use super::SafeExecutionReceipt;
use crate::connectors::{ConnectorDescriptor, ConnectorError};
use async_trait::async_trait;

#[async_trait]
pub trait ReceiptTransportConnector: Send + Sync {
    fn descriptor(&self) -> &ConnectorDescriptor;
    async fn publish(&self, receipt: &SafeExecutionReceipt) -> Result<(), ConnectorError>;
}
