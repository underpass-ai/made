use std::collections::VecDeque;

use made_core::ports::CeremonyStepHandlerRequest;
use made_core::value_objects::StepResult;
use tokio::sync::RwLock;

#[derive(Debug)]
pub(in crate::usecases) struct SequenceStepHandlerFake {
    pub(super) results: RwLock<VecDeque<StepResult>>,
    pub(super) requests: RwLock<Vec<CeremonyStepHandlerRequest>>,
}
