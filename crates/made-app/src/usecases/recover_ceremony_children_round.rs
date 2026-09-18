#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecoverCeremonyChildrenRound {
    pub recovered_plans: usize,
    pub accepted_completions: usize,
    pub skipped: usize,
    pub failed: usize,
    pub busy: bool,
}

impl RecoverCeremonyChildrenRound {
    #[must_use]
    pub const fn acknowledged(self) -> usize {
        self.recovered_plans + self.accepted_completions + self.skipped
    }
}
