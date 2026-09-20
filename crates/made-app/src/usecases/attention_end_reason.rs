//! Why one ask stopped when it did.

use serde::Serialize;

/// What ended the wait.
///
/// Three answers rather than an empty list, because "come back" and
/// "do not come back" look identical from the outside otherwise. A
/// host that read `Terminal` and kept asking would poll a finished
/// ceremony until somebody noticed the bill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionEndReason {
    /// There was work, and it is in this batch.
    Items,
    /// The wait ran out with nothing to hand over. Ask again.
    WaitElapsed,
    /// The ceremony reached an end, or the loop is stuck. Stop.
    Terminal,
}
