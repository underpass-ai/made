use super::ProtocolWorkerAdmissionDecision;

/// One deterministic bounded scheduling result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolWorkerSchedule {
    admitted: Vec<ProtocolWorkerAdmissionDecision>,
    deferred: Vec<ProtocolWorkerAdmissionDecision>,
}

impl ProtocolWorkerSchedule {
    #[must_use]
    pub const fn new(
        admitted: Vec<ProtocolWorkerAdmissionDecision>,
        deferred: Vec<ProtocolWorkerAdmissionDecision>,
    ) -> Self {
        Self { admitted, deferred }
    }
    #[must_use]
    pub fn admitted(&self) -> &[ProtocolWorkerAdmissionDecision] {
        &self.admitted
    }
    #[must_use]
    pub fn deferred(&self) -> &[ProtocolWorkerAdmissionDecision] {
        &self.deferred
    }
}
