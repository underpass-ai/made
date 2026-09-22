//! Everything the ledger holds for one bound integrator.

use made_core::error::DomainError;
use made_core::ports::{
    HostDeliveryLedgerPort, HostDeliveryPageLimit, HostDeliveryQuery, HostDeliveryTargetFilter,
};
use made_core::value_objects::{HostDeliveryRecord, HostDeliveryTarget};

/// How many pages of one binding's deliveries are worth walking.
///
/// A bound rather than a belief: the queue limit itself caps at a
/// thousand, and a binding whose ledger has grown past twenty pages is
/// over any limit an integrator could have asked for. Walking further
/// to be exact about *how* far over would cost the round for an answer
/// nobody acts on differently.
const MAX_PAGES: u32 = 20;

/// Reads one binding's whole ledger, a bounded number of pages of it.
///
/// Its own type because two very different questions need the same
/// walk — whether the queue is full, and whether the loop is getting
/// anywhere — and two walks written separately would page differently
/// the first time one of them was tuned.
pub struct BindingDeliveries<'ledger> {
    deliveries: &'ledger dyn HostDeliveryLedgerPort,
}

impl std::fmt::Debug for BindingDeliveries<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BindingDeliveries")
            .finish_non_exhaustive()
    }
}

impl<'ledger> BindingDeliveries<'ledger> {
    #[must_use]
    pub const fn new(deliveries: &'ledger dyn HostDeliveryLedgerPort) -> Self {
        Self { deliveries }
    }

    /// Every delivery addressed to this target, in ledger order.
    pub async fn all(
        &self,
        target: &HostDeliveryTarget,
    ) -> Result<Vec<HostDeliveryRecord>, DomainError> {
        let filter = HostDeliveryTargetFilter::any_of([target.clone()])?;
        let mut held = Vec::new();
        let mut cursor = None;
        for _ in 0..MAX_PAGES {
            let mut query = HostDeliveryQuery::new()
                .to(filter.clone())
                .of_size(HostDeliveryPageLimit::MAX);
            if let Some(after) = cursor {
                query = query.after(after);
            }
            let page = self.deliveries.list(&query).await?;
            held.extend(page.records().iter().cloned());
            match page.next_cursor() {
                Some(next) => cursor = Some(next.clone()),
                None => break,
            }
        }
        Ok(held)
    }
}
