use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{ProtocolStepHandlerPort, ProtocolStepHandlerRequest};
use made_core::value_objects::StepResult;

type StepHandlerFuture =
    Pin<Box<dyn Future<Output = Result<StepResult, DomainError>> + Send + 'static>>;
type StepHandlerCallback =
    dyn Fn(ProtocolStepHandlerRequest) -> StepHandlerFuture + Send + Sync + 'static;

/// Adapts an async host callback to [`ProtocolStepHandlerPort`].
///
/// The host receives a typed step request and returns a typed step result
/// without implementing a transport or running a MADE service.
#[derive(Clone)]
pub struct CallbackProtocolStepHandler {
    callback: Arc<StepHandlerCallback>,
}

impl CallbackProtocolStepHandler {
    #[must_use]
    pub fn new<F, Fut>(callback: F) -> Self
    where
        F: Fn(ProtocolStepHandlerRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<StepResult, DomainError>> + Send + 'static,
    {
        Self {
            callback: Arc::new(move |request| Box::pin(callback(request))),
        }
    }
}

impl fmt::Debug for CallbackProtocolStepHandler {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CallbackProtocolStepHandler")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl ProtocolStepHandlerPort for CallbackProtocolStepHandler {
    async fn execute(
        &self,
        request: ProtocolStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        (self.callback)(request).await
    }
}
