use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorCapability {
    Execute,
    Recover,
    ReadReceipt,
    WriteReceipt,
    AtomicReceipt,
    PublishReceipt,
    ConfirmDelivery,
}
