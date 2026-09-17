/// Number of one kind of element declared by a ceremony draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeremonyDraftElementCount(usize);

impl CeremonyDraftElementCount {
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}
