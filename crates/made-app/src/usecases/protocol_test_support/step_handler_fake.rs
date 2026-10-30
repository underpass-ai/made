use made_core::error::DomainError;
use made_core::ports::ProtocolStepHandlerRequest;
use made_core::value_objects::StepResult;
use tokio::sync::RwLock;

#[derive(Debug)]
pub(in crate::usecases) struct StepHandlerFake {
    pub(super) result: Result<StepResult, DomainError>,
    pub(super) requests: RwLock<Vec<ProtocolStepHandlerRequest>>,
}
