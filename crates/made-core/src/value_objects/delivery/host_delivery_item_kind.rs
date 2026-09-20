use std::fmt;

use serde::{Deserialize, Serialize};

/// Which kind of thing is being delivered to a host destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostDeliveryItemKind {
    /// A supervisor's question or instruction for a working agent.
    Intervention,
    /// A projected reason for an integrator to look at a ceremony.
    Attention,
}

impl HostDeliveryItemKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Intervention => "intervention",
            Self::Attention => "attention",
        }
    }
}

impl fmt::Display for HostDeliveryItemKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
