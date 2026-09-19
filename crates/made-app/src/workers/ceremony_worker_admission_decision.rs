use super::{
    CeremonyWorkerAdmissionReason, CeremonyWorkerPolicyVersion, CeremonyWorkerScheduleRequest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyWorkerAdmissionDecision {
    request: CeremonyWorkerScheduleRequest,
    reason: CeremonyWorkerAdmissionReason,
    policy_version: CeremonyWorkerPolicyVersion,
    sequence: u64,
}

impl CeremonyWorkerAdmissionDecision {
    #[must_use]
    pub const fn new(
        request: CeremonyWorkerScheduleRequest,
        reason: CeremonyWorkerAdmissionReason,
        policy_version: CeremonyWorkerPolicyVersion,
        sequence: u64,
    ) -> Self {
        Self {
            request,
            reason,
            policy_version,
            sequence,
        }
    }
    #[must_use]
    pub const fn request(&self) -> &CeremonyWorkerScheduleRequest {
        &self.request
    }
    #[must_use]
    pub const fn reason(&self) -> CeremonyWorkerAdmissionReason {
        self.reason
    }
    #[must_use]
    pub const fn policy_version(&self) -> CeremonyWorkerPolicyVersion {
        self.policy_version
    }
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
}
