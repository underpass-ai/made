use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CeremonyStateKind {
    Initial,
    Intermediate,
    Terminal,
}

impl CeremonyStateKind {
    #[must_use]
    pub fn is_initial(self) -> bool {
        self == Self::Initial
    }

    #[must_use]
    pub fn is_terminal(self) -> bool {
        self == Self::Terminal
    }
}
