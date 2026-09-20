use std::fmt;

use serde::{Deserialize, Serialize};

/// How a destination expects to be woken, as its binding declares it.
///
/// Distinct from which adapter the deployment has: a destination may
/// ask to be woken in a deployment with no adapter, and the honest
/// answer is that its deliveries wait to be pulled.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum HostActivationMode {
    /// Do not wake it; it asks for its own work.
    #[default]
    None,
    /// Wake it through the operator's configured command.
    Command,
}

impl HostActivationMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Command => "command",
        }
    }

    /// Whether a delivery to this destination should be pushed at all.
    #[must_use]
    pub const fn activates(self) -> bool {
        matches!(self, Self::Command)
    }
}

impl fmt::Display for HostActivationMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
