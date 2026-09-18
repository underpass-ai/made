use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CeremonyLifecyclePhase {
    #[default]
    Running,
    Paused,
    Ended,
}

impl CeremonyLifecyclePhase {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Ended => "ended",
        }
    }
}
