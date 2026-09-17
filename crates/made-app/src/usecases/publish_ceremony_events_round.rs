/// Result of one bounded publisher drain.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PublishCeremonyEventsRound {
    pub delivered: usize,
    /// Delivery attempts that failed but were retried automatically.
    ///
    /// This remains zero for [`super::PublishCeremonyEventsUseCase::execute`],
    /// which is one explicit round. The automatic path moves recovered
    /// failures here so callers do not mistake them for terminal failure.
    pub retried: usize,
    pub failed: usize,
    pub quarantined: usize,
    pub busy: bool,
}

impl PublishCeremonyEventsRound {
    #[must_use]
    pub const fn confirmed(self) -> usize {
        self.delivered + self.quarantined
    }
}
