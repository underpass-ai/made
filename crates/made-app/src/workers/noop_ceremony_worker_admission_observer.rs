use super::{CeremonyWorkerAdmissionDecision, CeremonyWorkerAdmissionObserver};

#[derive(Debug, Default)]
pub struct NoopCeremonyWorkerAdmissionObserver;

impl CeremonyWorkerAdmissionObserver for NoopCeremonyWorkerAdmissionObserver {
    fn observe(&self, _decision: &CeremonyWorkerAdmissionDecision) {}
}
