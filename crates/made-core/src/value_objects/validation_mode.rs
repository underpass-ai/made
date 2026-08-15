//! [`ValidationMode`] — how strictly a council decision honours its
//! configured [`OutputContract`].

use serde::{Deserialize, Serialize};

/// How the [`RunCouncilDecision`](crate::value_objects) RPC treats
/// validator failures when picking a winner.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Default)]
pub enum ValidationMode {
    /// Reject the deliberation if no candidate proposal satisfies the
    /// contract. Surfaces `DomainError::NoValidProposal` to the
    /// caller. This is the default and mirrors the historical
    /// [`Deliberation::pick_winner`] semantics when an
    /// [`OutputContract`] is configured.
    #[default]
    Strict,
    /// Always return the top-ranked candidate as the winner, even if
    /// it fails validation. The response carries a `passed=false`
    /// outcome and per-candidate validator reports so the caller can
    /// reason about the failure without losing the proposal.
    Warn,
}
