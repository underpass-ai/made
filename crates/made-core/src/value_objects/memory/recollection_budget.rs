use std::num::NonZeroUsize;

use serde::{Deserialize, Serialize};

/// How much of what memory holds a session is willing to read at its
/// opening, in bytes of summary.
///
/// A bound rather than a preference. A scope that a hundred sessions
/// have written to holds more than any opening can carry, and a
/// recollection that grew with the scope would make the hundred-and-
/// first session slower than the first for no benefit it could name.
///
/// Bytes of summary and not of the serialized payload: the summaries
/// are what a reader reads, and measuring the envelope would move the
/// budget every time a field was added beside them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RecollectionBudget(NonZeroUsize);

impl RecollectionBudget {
    /// Total, because a budget of nothing is not a budget: it is a
    /// recollection switched off, and switching one off is done by not
    /// declaring a shared scope.
    #[must_use]
    pub const fn of_bytes(bytes: NonZeroUsize) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn bytes(self) -> usize {
        self.0.get()
    }
}
