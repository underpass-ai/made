use std::fmt;

use serde::{Deserialize, Serialize};

/// Why a delivery stopped being worth making.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryExpiryCause {
    /// Nobody acknowledged it within the policy's own time.
    Timeout,
    /// The ceremony reached a terminal phase; the question is moot.
    CeremonyEnded,
    /// The destination stopped being one: the binding was revoked.
    TargetRevoked,
}

impl DeliveryExpiryCause {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::CeremonyEnded => "ceremony_ended",
            Self::TargetRevoked => "target_revoked",
        }
    }
}

impl fmt::Display for DeliveryExpiryCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
