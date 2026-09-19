use made_core::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CeremonyWorkerPolicyVersion(u64);

impl CeremonyWorkerPolicyVersion {
    pub fn new(value: u64) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "worker_policy_version",
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}
