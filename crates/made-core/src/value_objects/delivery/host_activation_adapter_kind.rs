use std::fmt;

use serde::{Deserialize, Serialize};

/// Which activation adapter a deployment has, if any.
///
/// Declared rather than inferred: a host that is never woken and a host
/// whose wake-up failed look the same from the outside, and an operator
/// deciding whether to poll needs to tell them apart.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum HostActivationAdapterKind {
    /// No adapter: hosts find their work by asking for it.
    #[default]
    None,
    /// The operator's own command, run with the envelope on its input.
    Command,
}

impl HostActivationAdapterKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Command => "command",
        }
    }
}

impl fmt::Display for HostActivationAdapterKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
