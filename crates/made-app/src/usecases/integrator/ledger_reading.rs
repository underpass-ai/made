//! What one binding's ledger says about the round it is in.

use made_core::value_objects::{
    AttentionEventId, GlobalPosition, HostDeliveryItem, HostDeliveryRecord, HostDeliveryStateKind,
    Owed,
};

/// The three facts a round is judged by, read in one pass.
///
/// All of them are about **this binding**. The head especially: a
/// cursor walks the global feed and advances over every ceremony in
/// the deployment, so a busy neighbour would move it every second and
/// a real stall would never be seen. The furthest record this binding
/// was actually offered something from is the head its own loop is
/// measured against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LedgerReading {
    /// The furthest record this binding has been offered something from.
    pub(super) head: Option<GlobalPosition>,
    /// How many of its deliveries a host has closed by acting on them.
    pub(super) closed: u32,
    /// Whether it is holding anything at all.
    pub(super) owed: Owed,
}

impl LedgerReading {
    pub(super) fn of(ledger: &[HostDeliveryRecord]) -> Self {
        Self {
            head: ledger.iter().filter_map(source_position).max(),
            closed: count(ledger, |record| {
                record.state().kind() == HostDeliveryStateKind::Processed
            }),
            owed: if ledger.iter().any(|record| !record.state().is_terminal()) {
                Owed::Something
            } else {
                Owed::Nothing
            },
        }
    }
}

/// Where the record behind one delivery sits in the global feed.
///
/// Read back out of the identity, which spells it in. A delivery whose
/// identity this build did not derive says nothing, and saying nothing
/// is right: it cannot be this binding's place in a feed it does not
/// recognise.
fn source_position(record: &HostDeliveryRecord) -> Option<GlobalPosition> {
    let HostDeliveryItem::Attention {
        ceremony_id,
        attention_id,
    } = record.item()
    else {
        return None;
    };
    AttentionEventId::source_position(attention_id, ceremony_id)
}

fn count(ledger: &[HostDeliveryRecord], wanted: impl Fn(&HostDeliveryRecord) -> bool) -> u32 {
    u32::try_from(ledger.iter().filter(|record| wanted(record)).count()).unwrap_or(u32::MAX)
}
