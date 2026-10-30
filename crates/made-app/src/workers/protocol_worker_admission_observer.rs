use super::ProtocolWorkerAdmissionDecision;

/// Read-only sink for admission decisions.
pub trait ProtocolWorkerAdmissionObserver: Send + Sync {
    fn observe(&self, decision: &ProtocolWorkerAdmissionDecision);
}
