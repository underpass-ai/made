use std::collections::BTreeMap;

use made_core::value_objects::{ExecutionIntent, ExecutionOperation, ExecutionReceipt};

#[derive(Debug, Default)]
pub(super) struct ExecutionReceiptStoreState {
    pub(super) operations: BTreeMap<String, ExecutionOperation>,
    pub(super) intents: BTreeMap<(String, String), ExecutionIntent>,
    pub(super) receipts: BTreeMap<String, ExecutionReceipt>,
}
