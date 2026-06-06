//! [`CeremonyStepHandlerPort`] — executes one ceremony step.

use async_trait::async_trait;

use crate::error::DomainError;
use crate::ports::CeremonyStepHandlerRequest;
use crate::value_objects::StepResult;

#[async_trait]
pub trait CeremonyStepHandlerPort: Send + Sync {
    async fn execute(&self, request: CeremonyStepHandlerRequest)
        -> Result<StepResult, DomainError>;
}
