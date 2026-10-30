use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::ProtocolEvidencePack;
use made_core::error::DomainError;
use made_core::ports::{ProtocolEvidenceRequest, ProtocolEvidenceSourcePort};

type EvidenceSourceFuture =
    Pin<Box<dyn Future<Output = Result<ProtocolEvidencePack, DomainError>> + Send + 'static>>;
type EvidenceSourceCallback =
    dyn Fn(ProtocolEvidenceRequest) -> EvidenceSourceFuture + Send + Sync + 'static;

/// Adapts an async host callback to [`ProtocolEvidenceSourcePort`].
#[derive(Clone)]
pub struct CallbackProtocolEvidenceSource {
    callback: Arc<EvidenceSourceCallback>,
}

impl CallbackProtocolEvidenceSource {
    #[must_use]
    pub fn new<F, Fut>(callback: F) -> Self
    where
        F: Fn(ProtocolEvidenceRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ProtocolEvidencePack, DomainError>> + Send + 'static,
    {
        Self {
            callback: Arc::new(move |request| Box::pin(callback(request))),
        }
    }
}

impl fmt::Debug for CallbackProtocolEvidenceSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CallbackProtocolEvidenceSource")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl ProtocolEvidenceSourcePort for CallbackProtocolEvidenceSource {
    async fn collect(
        &self,
        request: ProtocolEvidenceRequest,
    ) -> Result<ProtocolEvidencePack, DomainError> {
        (self.callback)(request).await
    }
}
