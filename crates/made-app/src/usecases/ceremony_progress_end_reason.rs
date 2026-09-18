/// Why a bounded ceremony progress stream stopped successfully.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeremonyProgressEndReason {
    Terminal,
    EventLimit,
    WaitElapsed,
}

impl CeremonyProgressEndReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::EventLimit => "event_limit",
            Self::WaitElapsed => "wait_elapsed",
        }
    }
}
