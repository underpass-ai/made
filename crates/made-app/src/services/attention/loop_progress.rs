//! What the delivery ledger says about a bound integrator's queue.

use serde::Serialize;

/// The ledger's half of the loop state.
///
/// `LoopState` is derived from two readings that live in different
/// places: what the ceremony says about itself, and what is still
/// owed to the host. This is the second, reduced to the only three
/// answers the first one needs, so the state machine does not have to
/// know how deliveries are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopProgress {
    /// Nothing outstanding: every delivery reached a processed end.
    Idle,
    /// Results are queued or delivered and not yet acted on.
    AwaitingResults,
    /// Something is stuck: an unprocessed blocking delivery, or the
    /// loop has gone its allowed rounds without moving.
    Blocked,
}
