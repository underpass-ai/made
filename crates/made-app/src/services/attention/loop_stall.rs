//! Why a loop stopped itself rather than going round again.

use serde::Serialize;

/// The two ways an integrator's loop runs out of road.
///
/// Both are read from the delivery ledger and neither is stored: a
/// stall is a property of what is owed and how often it has been
/// handed over, so a counter kept beside it would be a second truth
/// that a restart could disagree with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopStall {
    /// The same work has been handed over again and again and nothing
    /// has been acted on since.
    NoProgress,
    /// The loop has been round as many times as it was allowed.
    RoundLimit,
}

impl LoopStall {
    /// The name an operator reads, and the one the evidence records.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoProgress => "no_progress",
            Self::RoundLimit => "round_limit",
        }
    }

    /// Whether the loop may still be offered work it has not seen.
    ///
    /// A loop that has gone round its allowed number of times is done
    /// being offered results; one that is merely stuck is still told
    /// about news, because news is the thing most likely to unstick it.
    #[must_use]
    pub const fn admits_new_results(self) -> bool {
        matches!(self, Self::NoProgress)
    }
}
