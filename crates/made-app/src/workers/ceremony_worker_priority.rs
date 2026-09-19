use made_core::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CeremonyWorkerPriority(u16);

impl CeremonyWorkerPriority {
    pub const DEFAULT: Self = Self(0);
    pub const MAX: u16 = 1_000;
    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value > Self::MAX {
            return Err(DomainError::OutOfRange {
                field: "worker_priority",
                value: f64::from(value),
                min: 0.0,
                max: f64::from(Self::MAX),
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}
