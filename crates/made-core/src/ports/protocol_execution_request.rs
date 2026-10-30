use crate::ports::ProtocolStepHandlerRequest;
use crate::value_objects::ExecutionIntent;

/// Boundary request carrying both durable identity and handler-shaped work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolExecutionRequest {
    intent: ExecutionIntent,
    handler_request: ProtocolStepHandlerRequest,
}

impl ProtocolExecutionRequest {
    pub fn new(
        intent: ExecutionIntent,
        handler_request: ProtocolStepHandlerRequest,
    ) -> Result<Self, crate::DomainError> {
        if intent.operation().request() != &handler_request.semantic_request_bytes()? {
            return Err(crate::DomainError::InvariantViolated {
                reason: "handler request does not match the sealed execution request",
            });
        }
        Ok(Self {
            intent,
            handler_request,
        })
    }

    #[must_use]
    pub const fn intent(&self) -> &ExecutionIntent {
        &self.intent
    }

    #[must_use]
    pub const fn handler_request(&self) -> &ProtocolStepHandlerRequest {
        &self.handler_request
    }
}
