use async_trait::async_trait;
use made_core::entities::ProtocolEvidencePack;
use made_core::error::DomainError;
use made_core::ports::{ProtocolEvidenceRequest, ProtocolEvidenceSourcePort};

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopProtocolEvidenceSource;

impl NoopProtocolEvidenceSource {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProtocolEvidenceSourcePort for NoopProtocolEvidenceSource {
    async fn collect(
        &self,
        _request: ProtocolEvidenceRequest,
    ) -> Result<ProtocolEvidencePack, DomainError> {
        Err(DomainError::NotFound {
            what: "protocol_evidence_source",
        })
    }
}
