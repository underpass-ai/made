use made_core::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CeremonyWorkerWeight(u32);

impl CeremonyWorkerWeight {
    pub const MAX: u32 = 1_000;
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "worker_weight",
            });
        }
        if value > Self::MAX {
            return Err(DomainError::OutOfRange {
                field: "worker_weight",
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
