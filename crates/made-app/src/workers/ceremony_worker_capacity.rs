use made_core::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CeremonyWorkerCapacity(u32);

impl CeremonyWorkerCapacity {
    pub const MAX: u32 = 100_000;
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "worker_capacity",
            });
        }
        if value > Self::MAX {
            return Err(DomainError::OutOfRange {
                field: "worker_capacity",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(Self::MAX),
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}
