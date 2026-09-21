//! What one walk of the feed carries with it.

use std::collections::BTreeMap;

use made_core::entities::CeremonyDefinition;
use made_core::value_objects::{CeremonyId, HostDeliveryRecord};

use super::ProjectionRound;

/// The state one projection round threads through every record.
///
/// Its own type because four things travel together through every
/// record of a round and a function taking them one by one was already
/// at the limit: the queue as this round has changed it, the
/// definitions it has resolved, the one decision taken before it
/// started, and the tally it is filling in.
pub(super) struct ProjectionPass<'round> {
    pub(super) held: &'round mut Vec<HostDeliveryRecord>,
    pub(super) definitions: &'round mut BTreeMap<CeremonyId, Option<CeremonyDefinition>>,
    pub(super) withholding: bool,
    pub(super) tally: &'round mut ProjectionRound,
}
