/// What happened to one element between two ceremony definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CeremonyChangeKind {
    Added,
    Removed,
    Altered,
}

impl CeremonyChangeKind {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Altered => "altered",
        }
    }
}
