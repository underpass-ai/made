use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::value_objects::{AuthorizedOperation, ExecutionReceipt};

use super::ExecutionReconciliationAuditRecord;

/// Boundary for encoding, storing and protecting an authorized reconciliation attempt.
#[async_trait]
pub trait ExecutionReconciliationAuditPort: Send + Sync {
    async fn record_authorized_attempt(
        &self,
        authorization: &AuthorizedOperation,
        receipt: &ExecutionReceipt,
    ) -> Result<ExecutionReconciliationAuditRecord, DomainError>;

    async fn protect_authorized_attempt(
        &self,
        audit: &ExecutionReconciliationAuditRecord,
    ) -> Result<(), DomainError>;
}
