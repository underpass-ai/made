use crate::entities::ceremony_events::ExecutionReceiptLinked;
use crate::entities::CeremonyInstance;

impl CeremonyInstance {
    pub(super) fn apply_execution_receipt_linked(&mut self, event: &ExecutionReceiptLinked) {
        self.execution_receipt_links
            .entry(event.link.operation_id().clone())
            .or_insert_with(|| event.link.clone());
        self.updated_at = event.linked_at;
    }
}
