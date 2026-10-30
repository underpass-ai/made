/// What happened to one element between two protocol definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProtocolChangeKind {
    Added,
    Removed,
    Altered,
}

impl ProtocolChangeKind {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Altered => "altered",
        }
    }
}
