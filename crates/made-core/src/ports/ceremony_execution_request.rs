use crate::ports::CeremonyStepHandlerRequest;
use crate::value_objects::ExecutionIntent;

/// Boundary request carrying both durable identity and handler-shaped work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyExecutionRequest {
    intent: ExecutionIntent,
    handler_request: CeremonyStepHandlerRequest,
}

impl CeremonyExecutionRequest {
    #[must_use]
    pub const fn new(intent: ExecutionIntent, handler_request: CeremonyStepHandlerRequest) -> Self {
        Self {
            intent,
            handler_request,
        }
    }

    #[must_use]
    pub const fn intent(&self) -> &ExecutionIntent {
        &self.intent
    }

    #[must_use]
    pub const fn handler_request(&self) -> &CeremonyStepHandlerRequest {
        &self.handler_request
    }
}
