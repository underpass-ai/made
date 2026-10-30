use super::{
    ProtocolWorkerAdmissionReason, ProtocolWorkerPolicyVersion, ProtocolWorkerScheduleRequest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolWorkerAdmissionDecision {
    request: ProtocolWorkerScheduleRequest,
    reason: ProtocolWorkerAdmissionReason,
    policy_version: ProtocolWorkerPolicyVersion,
    sequence: u64,
}

impl ProtocolWorkerAdmissionDecision {
    #[must_use]
    pub const fn new(
        request: ProtocolWorkerScheduleRequest,
        reason: ProtocolWorkerAdmissionReason,
        policy_version: ProtocolWorkerPolicyVersion,
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
    pub const fn request(&self) -> &ProtocolWorkerScheduleRequest {
        &self.request
    }
    #[must_use]
    pub const fn reason(&self) -> ProtocolWorkerAdmissionReason {
        self.reason
    }
    #[must_use]
    pub const fn policy_version(&self) -> ProtocolWorkerPolicyVersion {
        self.policy_version
    }
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
}
