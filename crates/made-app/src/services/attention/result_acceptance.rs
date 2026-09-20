//! Whether the result behind an attention event is sealed or merely
//! reported.

use serde::Serialize;

/// How much authority stands behind a result an integrator is told about.
///
/// The loop wakes a host for two different kinds of news, and telling
/// them apart is the whole point: a step completed in the journal is a
/// fact the ceremony decided, while a participant reporting itself
/// finished is a claim nobody has sealed yet. An integrator that
/// treated the second as the first would register work that never
/// happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultAcceptance {
    /// Sealed in the journal; the ceremony has decided this.
    Accepted,
    /// Reported by a host, not yet sealed. The step is still open, and
    /// somebody has to complete it before the result counts.
    PendingRegistration,
    /// The event carries no result to accept, such as a ceremony ending.
    NotApplicable,
}

impl ResultAcceptance {
    /// Whether acting on this needs a registration step first.
    #[must_use]
    pub const fn needs_registration(self) -> bool {
        matches!(self, Self::PendingRegistration)
    }
}
