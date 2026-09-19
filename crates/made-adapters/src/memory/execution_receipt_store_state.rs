use std::collections::BTreeMap;

use made_core::value_objects::{
    ExecutionIntent, ExecutionOperation, ExecutionReceipt, ExecutionReconciliationRequirement,
};

/// Mutable rows guarded by the in-memory execution receipt store.
#[derive(Debug, Default)]
pub(super) struct ExecutionReceiptStoreState {
    pub(super) operations: BTreeMap<String, ExecutionOperation>,
    pub(super) intents: BTreeMap<(String, String), ExecutionIntent>,
    pub(super) reconciliation_requirements:
        BTreeMap<(String, String), ExecutionReconciliationRequirement>,
    pub(super) receipts: BTreeMap<String, ExecutionReceipt>,
}
