//! [`AttentionSubscriber`] — an append becomes a knock on the
//! integrator's door.
//!
//! # Why a subscriber beside a durable cursor
//!
//! The projector is a durable consumer and does not need to be told
//! anything: reopen the store and it walks on from where its cursor
//! stopped. What it needs is a reason to walk now. Without one, the
//! loop would only move when a host happened to ask, and a host that
//! is waiting for news it has not been told about has no reason to
//! ask. So the append wakes the projection, and the cursor is what
//! makes a missed wake-up harmless rather than lost.
//!
//! # Nothing here can fail an append
//!
//! A projection that did not run is work the loop is still owed, not a
//! ceremony that did not happen. The failure is logged with the
//! ceremony it was walking for, and the next append — or the next
//! `await` or `list`, which project before they read — picks the
//! cursor up where it was left.

use std::sync::Arc;

use async_trait::async_trait;
use made_core::ports::{CeremonyEventSubscriberPort, PositionedRecord};

use super::AttentionRecovery;

/// Wakes the attention projection after every append.
pub struct AttentionSubscriber {
    recovery: Arc<AttentionRecovery>,
}

impl std::fmt::Debug for AttentionSubscriber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AttentionSubscriber")
            .finish_non_exhaustive()
    }
}

impl AttentionSubscriber {
    #[must_use]
    pub const fn new(recovery: Arc<AttentionRecovery>) -> Self {
        Self { recovery }
    }
}

#[async_trait]
impl CeremonyEventSubscriberPort for AttentionSubscriber {
    /// One round per append, not one per record.
    ///
    /// An append targets one ceremony and the projector walks the feed
    /// by position, so the records of this append are reached by
    /// walking whatever the cursor had not reached yet. Projecting
    /// once per record would repeat that walk for no new news.
    async fn observe(&self, records: &[PositionedRecord]) {
        let Some(positioned) = records.first() else {
            return;
        };
        let ceremony_id = positioned.record.ceremony_id();
        if let Err(error) = self.recovery.for_ceremony(ceremony_id).await {
            tracing::warn!(
                ceremony_id = %ceremony_id,
                position = ?positioned.position,
                %error,
                "attention projection failed after an append; the cursor keeps the work owed"
            );
        }
    }
}

#[cfg(test)]
mod tests;
