/// Whether a definition change could strand an already-running protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProtocolChangeImpact {
    Carries,
    Strands,
}

impl ProtocolChangeImpact {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Carries => "carries",
            Self::Strands => "strands",
        }
    }

    #[must_use]
    pub const fn strands(self) -> bool {
        matches!(self, Self::Strands)
    }
}
