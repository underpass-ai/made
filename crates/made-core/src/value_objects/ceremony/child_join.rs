use serde::{Deserialize, Serialize};

use super::ChildQuorum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChildJoin {
    All,
    Any,
    Quorum { count: ChildQuorum },
}

impl ChildJoin {
    #[must_use]
    pub fn is_satisfied(self, completed: usize, total: usize) -> bool {
        match self {
            Self::All => total > 0 && completed == total,
            Self::Any => completed > 0,
            Self::Quorum { count } => completed >= usize::from(count.get()),
        }
    }
}
