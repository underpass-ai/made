use serde::{Deserialize, Serialize};

/// How the deployable composition root selects session memory.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemorySelection {
    /// Use SQLite when a ceremony-store path is present; otherwise forget.
    #[default]
    Automatic,
    /// Use the SQLite ceremony store and refuse a missing path.
    Sqlite,
    /// Deliberately keep no memory between sessions.
    None,
}
