/// Number of one kind of element declared by a protocol draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolDraftElementCount(usize);

impl ProtocolDraftElementCount {
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}
