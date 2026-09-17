/// Result of one bounded publisher drain.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PublishCeremonyEventsRound {
    pub delivered: usize,
    pub failed: usize,
    pub quarantined: usize,
    pub busy: bool,
}
