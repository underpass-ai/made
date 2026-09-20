//! What one walk of the feed did, for the caller that has to decide
//! whether to walk it again.

use serde::Serialize;

/// The tally of a single bounded projection round.
///
/// Counted rather than logged, because the two questions asked of a
/// durable consumer are answered by different numbers: "is it keeping
/// up" is `read`, and "is anything reaching the host" is `queued` plus
/// `activated`. A round that read a hundred records and queued nothing
/// is healthy; one that queued a hundred and activated none means the
/// deployment does not wake hosts, which is a legitimate configuration
/// and a bad surprise.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ProjectionRound {
    /// Records the cursor advanced past, whatever came of them.
    pub read: u32,
    /// Deliveries the ledger did not already hold.
    pub queued: u32,
    /// Deliveries the ledger recognised: replaying the feed is free.
    pub already_held: u32,
    /// Envelopes an activation adapter accepted.
    pub activated: u32,
    /// Items shed to keep one binding's queue bounded.
    pub shed: u32,
    /// Activations the adapter tried and could not make.
    pub failed: u32,
    /// Another worker holds this consumer's cursor.
    pub busy: bool,
}

impl ProjectionRound {
    /// Whether the round is worth repeating straight away.
    ///
    /// A round that read nothing has caught up with the feed; one that
    /// found the cursor busy is somebody else's turn. Both mean stop,
    /// for different reasons, and a caller that cannot tell them apart
    /// spins.
    #[must_use]
    pub const fn made_progress(&self) -> bool {
        self.read > 0 && !self.busy
    }
}
