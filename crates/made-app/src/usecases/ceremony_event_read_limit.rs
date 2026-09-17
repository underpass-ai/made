/// Maximum number of ceremony records returned by one read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeremonyEventReadLimit(usize);

impl CeremonyEventReadLimit {
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}
