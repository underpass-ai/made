//! Keeping one integrator's queue bounded.

use made_core::error::DomainError;
use made_core::ports::{
    HostDeliveryLedgerPort, HostDeliveryPageLimit, HostDeliveryQuery, HostDeliveryTargetFilter,
};
use made_core::value_objects::{
    AttentionKind, DeliveryExpiryCause, HostDeliveryItem, HostDeliveryRecord,
};
use time::OffsetDateTime;

use super::AttentionAudience;

/// How many pages of one binding's deliveries are worth walking before
/// treating the queue as full on the evidence so far.
///
/// A bound rather than a belief: the queue limit itself caps at a
/// thousand, and a binding whose ledger has grown past twenty pages of
/// outstanding work is over any limit an integrator could have asked
/// for. Scanning further to be exact about *how* over would cost the
/// round for an answer nobody acts on differently.
const MAX_PAGES: u32 = 20;

/// Sheds what a full queue cannot hold.
///
/// Its own type because it is the one place in the loop that decides
/// something is not worth telling a host. That decision is not the
/// projector's reading of the feed and must not be tangled with it:
/// the rules say what happened, and this says what there is room to
/// say.
pub(super) struct AttentionBackpressure<'ledger> {
    deliveries: &'ledger dyn HostDeliveryLedgerPort,
}

impl<'ledger> AttentionBackpressure<'ledger> {
    pub(super) const fn new(deliveries: &'ledger dyn HostDeliveryLedgerPort) -> Self {
        Self { deliveries }
    }

    /// Make room for one more item, and say what that cost.
    ///
    /// Returns how many offers were given up on. Zero is the ordinary
    /// answer; anything else is news in its own right, because a host
    /// that silently stops being told about results cannot tell that
    /// from a ceremony that went quiet.
    ///
    /// A queue full of things nobody may drop — a human decision, a
    /// blocked signal, an ending — is left alone and the new item goes
    /// through. The limit is backpressure, not a cap on decisions: a
    /// loop that refused to deliver an ending because it was busy
    /// would hang waiting for the ending it refused.
    pub(super) async fn make_room(
        &self,
        audience: &AttentionAudience,
        now: OffsetDateTime,
    ) -> Result<u32, DomainError> {
        let waiting = self.outstanding(audience, now).await?;
        let limit = audience.policy().max_queued().value() as usize;
        if waiting.len() < limit {
            return Ok(0);
        }
        // One more has to fit, so the queue has to end up below the
        // limit rather than at it.
        let must_go = waiting.len() + 1 - limit;
        let mut shed = 0;
        for victim in oldest_first_droppable(&waiting).into_iter().take(must_go) {
            if self
                .deliveries
                .abandon(victim.id(), DeliveryExpiryCause::QueueOverflow, now)
                .await?
                .is_some()
            {
                shed += 1;
            }
        }
        Ok(shed)
    }

    /// Everything this binding has been offered and not yet taken.
    async fn outstanding(
        &self,
        audience: &AttentionAudience,
        now: OffsetDateTime,
    ) -> Result<Vec<HostDeliveryRecord>, DomainError> {
        let target = HostDeliveryTargetFilter::any_of([audience.binding().delivery_target()])?;
        let mut waiting = Vec::new();
        let mut cursor = None;
        for _ in 0..MAX_PAGES {
            let mut query = HostDeliveryQuery::new()
                .to(target.clone())
                .of_size(HostDeliveryPageLimit::MAX);
            if let Some(after) = cursor {
                query = query.after(after);
            }
            let page = self.deliveries.list(&query).await?;
            waiting.extend(
                page.records()
                    .iter()
                    .filter(|record| record.is_offerable_at(now))
                    .cloned(),
            );
            match page.next_cursor() {
                Some(next) => cursor = Some(next.clone()),
                None => break,
            }
        }
        Ok(waiting)
    }
}

/// The offers a full queue may give up on, oldest first.
///
/// Sorted here rather than trusted from the store, which pages by
/// identity: "the oldest" is a fact about when the offer was made, and
/// identity order is alphabetical accident.
fn oldest_first_droppable(waiting: &[HostDeliveryRecord]) -> Vec<&HostDeliveryRecord> {
    let mut droppable: Vec<&HostDeliveryRecord> = waiting
        .iter()
        .filter(|record| kind_of(record.item()).is_some_and(|kind| !kind.is_blocking()))
        .collect();
    droppable.sort_by_key(|record| record.created_at());
    droppable
}

/// What a queued delivery is about, read back from its identity.
///
/// The ledger holds an attention event's identifier, not the event,
/// and that identifier ends in the kind that derived it. A delivery
/// whose identity does not end in a kind this build knows is left
/// alone: shedding something the engine cannot name is how a queue
/// quietly loses the thing it should not have.
fn kind_of(item: &HostDeliveryItem) -> Option<AttentionKind> {
    match item {
        HostDeliveryItem::Attention { attention_id, .. } => {
            AttentionKind::from_label(attention_id.as_str().rsplit(':').next()?)
        }
        HostDeliveryItem::Intervention { .. } => None,
    }
}
