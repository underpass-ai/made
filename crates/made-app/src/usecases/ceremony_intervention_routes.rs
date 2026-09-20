use std::collections::BTreeMap;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{HostDeliveryLedgerPort, HostDeliveryQuery};
use made_core::value_objects::{
    CeremonyId, CeremonyInterventionId, HostDeliveryItem, HostDeliveryRecord,
};

/// Every route one ceremony's interventions have taken, by item.
///
/// Read in one pass rather than per item, because the questions a
/// caller asks — show me this item, show me the unresolved ones — all
/// need the same page of the ledger, and asking once per intervention
/// turns a listing into a fan of round trips against the store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CeremonyInterventionRoutes {
    by_intervention: BTreeMap<CeremonyInterventionId, Vec<HostDeliveryRecord>>,
}

/// Bounded so a ceremony with a runaway number of routes cannot turn
/// one read into an unbounded walk of the ledger.
const MAX_PAGES: usize = 20;

impl CeremonyInterventionRoutes {
    pub async fn load(
        ledger: &Arc<dyn HostDeliveryLedgerPort>,
        ceremony_id: &CeremonyId,
    ) -> Result<Self, DomainError> {
        let mut by_intervention: BTreeMap<CeremonyInterventionId, Vec<HostDeliveryRecord>> =
            BTreeMap::new();
        let mut query = HostDeliveryQuery::new().in_ceremony(ceremony_id.clone());
        for _ in 0..MAX_PAGES {
            let page = ledger.list(&query).await?;
            let next = page.next_cursor().cloned();
            for record in page.into_records() {
                if let HostDeliveryItem::Intervention {
                    intervention_id, ..
                } = record.item()
                {
                    by_intervention
                        .entry(intervention_id.clone())
                        .or_default()
                        .push(record);
                }
            }
            match next {
                Some(cursor) => query = query.after(cursor),
                None => break,
            }
        }
        Ok(Self { by_intervention })
    }

    #[must_use]
    pub fn of(&self, intervention_id: &CeremonyInterventionId) -> &[HostDeliveryRecord] {
        self.by_intervention
            .get(intervention_id)
            .map_or(&[], Vec::as_slice)
    }
}
