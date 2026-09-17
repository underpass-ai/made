use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateExecution {
    #[default]
    Sequential,
    Concurrent,
}

impl StateExecution {
    #[must_use]
    pub const fn is_concurrent(self) -> bool {
        matches!(self, Self::Concurrent)
    }

    #[must_use]
    pub const fn is_sequential(&self) -> bool {
        matches!(self, Self::Sequential)
    }
}
