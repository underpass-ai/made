use super::CeremonyWorkerAdmissionDecision;

/// One deterministic bounded scheduling result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyWorkerSchedule {
    admitted: Vec<CeremonyWorkerAdmissionDecision>,
    deferred: Vec<CeremonyWorkerAdmissionDecision>,
}

impl CeremonyWorkerSchedule {
    #[must_use]
    pub const fn new(
        admitted: Vec<CeremonyWorkerAdmissionDecision>,
        deferred: Vec<CeremonyWorkerAdmissionDecision>,
    ) -> Self {
        Self { admitted, deferred }
    }
    #[must_use]
    pub fn admitted(&self) -> &[CeremonyWorkerAdmissionDecision] {
        &self.admitted
    }
    #[must_use]
    pub fn deferred(&self) -> &[CeremonyWorkerAdmissionDecision] {
        &self.deferred
    }
}
