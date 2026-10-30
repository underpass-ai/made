use async_trait::async_trait;

use crate::entities::ProtocolEvidencePack;
use crate::error::DomainError;

use super::ProtocolEvidenceRequest;

#[async_trait]
pub trait ProtocolEvidenceSourcePort: Send + Sync {
    async fn collect(
        &self,
        request: ProtocolEvidenceRequest,
    ) -> Result<ProtocolEvidencePack, DomainError>;
}
