//! [`ProtocolStepHandlerPort`] — executes one protocol step.

use async_trait::async_trait;

use crate::error::DomainError;
use crate::ports::ProtocolStepHandlerRequest;
use crate::value_objects::StepResult;

#[async_trait]
pub trait ProtocolStepHandlerPort: Send + Sync {
    async fn execute(&self, request: ProtocolStepHandlerRequest)
        -> Result<StepResult, DomainError>;
}
