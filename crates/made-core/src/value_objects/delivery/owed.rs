use serde::{Deserialize, Serialize};

/// Whether the binding was holding work when it asked.
///
/// A loop with an empty queue that asks and is told nothing is
/// waiting, which is what a loop does while an agent works a long step
/// or a person thinks. A loop holding work it was handed and told
/// nothing is the one that is going round without moving. The spec's
/// "rounds without progress" is the second, and only the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Owed {
    /// Nothing outstanding: every delivery reached an end.
    Nothing,
    /// At least one delivery offered, handed over or acknowledged and
    /// not yet closed.
    Something,
}
