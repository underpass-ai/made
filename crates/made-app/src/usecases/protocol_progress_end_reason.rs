/// Why a bounded protocol progress stream stopped successfully.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolProgressEndReason {
    Terminal,
    EventLimit,
    WaitElapsed,
}

impl ProtocolProgressEndReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::EventLimit => "event_limit",
            Self::WaitElapsed => "wait_elapsed",
        }
    }
}
