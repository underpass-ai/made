//! Turning a delivery the ledger holds back into the news it stands for.

use made_core::error::DomainError;
use made_core::ports::CeremonyEventStorePort;
use made_core::value_objects::{
    AttentionEventId, CeremonyEventPageLimit, CeremonyId, HostDeliveryItem, RoleId,
};

use super::{attention_for, queue_overflow, AttentionEvent};

/// Derive again the attention event one delivery was opened for.
///
/// The ledger holds an identity, never the event: attention is a
/// reading of the journal, and a second copy of it would be a second
/// truth the moment a projection rule changed. So handing a host what
/// it was woken for means reading the record again and deriving it
/// again, which is also what makes the answer honest — a host is told
/// what the rules say *now*, not what they said when the delivery was
/// queued.
///
/// `None` means the delivery no longer stands for anything this build
/// can name: an identity from somewhere else, a record the store has
/// dropped, or a rule that has since stopped producing that reading.
/// The caller hands the lease back rather than inventing something.
pub async fn replay(
    events: &dyn CeremonyEventStorePort,
    item: &HostDeliveryItem,
    integrator: &RoleId,
) -> Result<Option<AttentionEvent>, DomainError> {
    let HostDeliveryItem::Attention {
        ceremony_id,
        attention_id,
    } = item
    else {
        return Ok(None);
    };
    let Some(position) = attention_id.source_position(ceremony_id) else {
        return Ok(None);
    };
    let one = CeremonyEventPageLimit::new(1)?;
    let Some(record) = events.read_all(position, one).await?.into_iter().next() else {
        return Ok(None);
    };
    if record.position != position || record.record.ceremony_id() != ceremony_id {
        // The feed no longer has that record at that position. Better
        // to say nothing than to wake a host about somebody else's
        // ceremony because an identity pointed at a moved target.
        return Ok(None);
    }
    // Two derivations can stand behind one record: what the rules read
    // in it, and the overflow marker the projector derives from
    // whichever record was arriving when the queue filled up. The
    // identity says which of the two this delivery is.
    if let Some(derived) = attention_for(&record, integrator)? {
        if matches(&derived, attention_id, ceremony_id) {
            return Ok(Some(derived));
        }
    }
    let overflow = queue_overflow(&record)?;
    Ok(matches(&overflow, attention_id, ceremony_id).then_some(overflow))
}

fn matches(event: &AttentionEvent, id: &AttentionEventId, ceremony_id: &CeremonyId) -> bool {
    event.id() == id && event.ceremony_id() == ceremony_id
}
