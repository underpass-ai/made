use super::CeremonyWorkerAdmissionDecision;

/// Read-only sink for admission decisions.
pub trait CeremonyWorkerAdmissionObserver: Send + Sync {
    fn observe(&self, decision: &CeremonyWorkerAdmissionDecision);
}
