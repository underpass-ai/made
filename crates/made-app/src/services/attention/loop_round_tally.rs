//! How many times one binding has been round, read from the ledger.
//!
//! Named for the tally rather than for the rounds, because
//! [`made_core::value_objects::LoopRounds`] is a ceiling a composed
//! system sets and this is a count of what happened.

use made_core::value_objects::{HostDeliveryRecord, HostDeliveryStateKind};
use serde::Serialize;
use time::OffsetDateTime;

/// The two counts a stall is decided from.
///
/// Both come out of the delivery ledger, which is durable, so they
/// survive a restart without anybody keeping a round counter. That
/// matters more than it looks: a counter in memory would reset with
/// the process and a loop that had been stuck for an hour would come
/// back looking fresh.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct LoopRoundTally {
    /// Every time this binding was handed something, over its life.
    handed_over: u32,
    /// The most times one outstanding item has been handed over with
    /// nothing acted on since.
    stuck: u32,
}

impl LoopRoundTally {
    /// Read both counts from everything the ledger holds for a binding.
    ///
    /// A round is one hand-over, which the ledger records as a lease
    /// rather than as an attempt: an attempt is what a *failed*
    /// delivery counts, and a host that keeps taking the same item and
    /// never closing it fails nothing.
    ///
    /// Only hand-overs at or after the last time this binding closed
    /// anything count as stuck. A host that took something else up in
    /// the meantime is working, and calling that a stall would stop a
    /// loop that is moving — slowly, and in the right direction.
    ///
    /// Bounded by what a delivery's history keeps, which is the last
    /// [`DeliveryHistory::CAPACITY`] transitions. A limit higher than
    /// that could not be reached on one item, and no limit anybody
    /// would set comes close.
    ///
    /// [`DeliveryHistory::CAPACITY`]: made_core::value_objects::DeliveryHistory::CAPACITY
    #[must_use]
    pub fn read(deliveries: &[HostDeliveryRecord], now: OffsetDateTime) -> Self {
        let last_processed = deliveries
            .iter()
            .filter_map(|record| last_entry_at(record, HostDeliveryStateKind::Processed))
            .max()
            .unwrap_or(OffsetDateTime::UNIX_EPOCH);
        let handed_over = deliveries
            .iter()
            .map(|record| hand_overs_since(record, OffsetDateTime::UNIX_EPOCH))
            .sum();
        let stuck = deliveries
            .iter()
            .filter(|record| record.is_offerable_at(now))
            .map(|record| hand_overs_since(record, last_processed))
            .max()
            .unwrap_or(0);
        Self { handed_over, stuck }
    }

    /// Rounds this binding has been handed something, ever.
    #[must_use]
    pub const fn handed_over(self) -> u32 {
        self.handed_over
    }

    /// Consecutive rounds spent on work nothing came of.
    #[must_use]
    pub const fn stuck(self) -> u32 {
        self.stuck
    }
}

/// The last time this delivery was in a given state, if it ever was.
fn last_entry_at(
    record: &HostDeliveryRecord,
    kind: HostDeliveryStateKind,
) -> Option<OffsetDateTime> {
    record
        .history()
        .entries()
        .iter()
        .filter(|entry| entry.state_kind() == kind)
        .map(|entry| entry.at())
        .max()
}

/// How many times this delivery was handed to a host at or after `since`.
fn hand_overs_since(record: &HostDeliveryRecord, since: OffsetDateTime) -> u32 {
    u32::try_from(
        record
            .history()
            .entries()
            .iter()
            .filter(|entry| entry.state_kind() == HostDeliveryStateKind::Leased)
            .filter(|entry| entry.at() >= since)
            .count(),
    )
    .unwrap_or(u32::MAX)
}
