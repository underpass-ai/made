use super::{ProtocolWorkerAdmissionDecision, ProtocolWorkerAdmissionObserver};

#[derive(Debug, Default)]
pub struct NoopProtocolWorkerAdmissionObserver;

impl ProtocolWorkerAdmissionObserver for NoopProtocolWorkerAdmissionObserver {
    fn observe(&self, _decision: &ProtocolWorkerAdmissionDecision) {}
}
