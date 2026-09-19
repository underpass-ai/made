use crate::entities::ceremony_events::ExecutionReceiptLinked;
use crate::entities::CeremonyInstance;
use crate::value_objects::ExecutionReceiptLinkKind;

impl CeremonyInstance {
    pub(super) fn apply_execution_receipt_linked(&mut self, event: &ExecutionReceiptLinked) {
        let operation_id = event.link.operation_id().clone();
        if self.execution_receipt_links.contains_key(&operation_id)
            && event.link.kind() == ExecutionReceiptLinkKind::Adopted
        {
            self.execution_receipt_adoptions
                .entry(operation_id)
                .or_default()
                .entry(event.link.applied_claim_fence().clone())
                .or_insert_with(|| event.link.clone());
        } else {
            self.execution_receipt_links
                .entry(operation_id)
                .or_insert_with(|| event.link.clone());
        }
        self.updated_at = event.linked_at;
    }
}
