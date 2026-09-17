use serde::{Deserialize, Serialize};

use crate::RecalledEntryView;

/// What earlier sessions in a ceremony's memory scope decided, as that
/// ceremony was told when it opened.
///
/// Absent from a summary when the ceremony was told nothing, which is
/// every ceremony that declares no `memory_scope` in its context: the
/// default scope is the ceremony's own id and nobody else writes there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecollectionView {
    /// The memory this ceremony reads and writes, as `kind:name`.
    pub scope: String,
    pub entries: Vec<RecalledEntryView>,
    /// Whether the budget stopped the rendering before the scope ran
    /// out. A consumer weighing what earlier sessions decided has to
    /// know whether it is looking at the whole record or the front of
    /// it.
    pub truncated: bool,
}
