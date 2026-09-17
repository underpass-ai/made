//! [`CeremonyEventSubscriberPort`] — the one seam every projection
//! hangs off.
//!
//! A ceremony is its event stream (ADR-012), and everything else — the
//! transcript, the report, what a session leaves behind in memory,
//! what an operator reads off a dashboard — is a projection of it. A
//! projection that learned about a session from the use case that
//! advanced it would be a second place the truth lives, and the two
//! would drift the day a fourth caller appended without telling it. So
//! there is one place to be told, and it is here.
//!
//! # A subscriber cannot fail an append
//!
//! The signature returns nothing. Not a swallowed `Result`, which
//! would let an author believe a failure goes somewhere: a subscriber
//! that has something to report logs it at warning level with the
//! ceremony id and the sequence it was looking at, and returns. The
//! facts landed before this was called, and a projection that cannot
//! be built is rebuilt from the stream it was derived from.
//!
//! # This is not how a durable consumer reads
//!
//! Publication and anything else that must not miss a record keeps its
//! own cursor over the global order and reads the stream through it
//! (plan §3.1, slice A6). A subscriber is told once, in process,
//! immediately; that is the right shape for a projection computed on
//! read and the wrong one for delivery.

use async_trait::async_trait;

use crate::ports::PositionedRecord;

/// Told about the records of one successful append, in order.
#[async_trait]
pub trait CeremonyEventSubscriberPort: Send + Sync {
    /// Observe what one append sealed.
    ///
    /// Every record belongs to the same stream — an append targets one
    /// ceremony — and they arrive in the order they were sealed, each
    /// with the place the store filed it in the order every stream
    /// shares. Called once per append that landed, after the store
    /// confirmed it; an append that conflicted or failed sealed
    /// nothing and there is nothing to observe.
    async fn observe(&self, records: &[PositionedRecord]);
}
