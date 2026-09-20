/// Whether binding an integrator may displace one that is already there.
///
/// Named rather than a bare flag: the two answers are "take over" and
/// "tell me who has it", and a call that guessed wrong would either
/// steal a live host's work or refuse a legitimate hand-over.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BindReplacement {
    /// Refuse if a different binding is live for this scope.
    #[default]
    Refuse,
    /// Displace the live binding and raise the fence.
    Replace,
}

impl BindReplacement {
    #[must_use]
    pub const fn replaces(self) -> bool {
        matches!(self, Self::Replace)
    }
}
