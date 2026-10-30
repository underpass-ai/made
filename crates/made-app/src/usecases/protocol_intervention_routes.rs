use std::collections::BTreeMap;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{HostDeliveryLedgerPort, HostDeliveryQuery};
use made_core::value_objects::{
    HostDeliveryItem, HostDeliveryRecord, ProtocolId, ProtocolInterventionId,
};

/// Every route one protocol's interventions have taken, by item.
///
/// Read in one pass rather than per item, because the questions a
/// caller asks — show me this item, show me the unresolved ones — all
/// need the same page of the ledger, and asking once per intervention
/// turns a listing into a fan of round trips against the store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProtocolInterventionRoutes {
    by_intervention: BTreeMap<ProtocolInterventionId, Vec<HostDeliveryRecord>>,
}

/// Bounded so a protocol with a runaway number of routes cannot turn
/// one read into an unbounded walk of the ledger.
const MAX_PAGES: usize = 20;

impl ProtocolInterventionRoutes {
    pub async fn load(
        ledger: &Arc<dyn HostDeliveryLedgerPort>,
        protocol_id: &ProtocolId,
    ) -> Result<Self, DomainError> {
        let mut by_intervention: BTreeMap<ProtocolInterventionId, Vec<HostDeliveryRecord>> =
            BTreeMap::new();
        let mut query = HostDeliveryQuery::new().in_protocol(protocol_id.clone());
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
    pub fn of(&self, intervention_id: &ProtocolInterventionId) -> &[HostDeliveryRecord] {
        self.by_intervention
            .get(intervention_id)
            .map_or(&[], Vec::as_slice)
    }
}
