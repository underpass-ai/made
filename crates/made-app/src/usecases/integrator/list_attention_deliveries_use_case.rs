//! [`ListAttentionDeliveriesUseCase`] — what the loop has been offered,
//! and what became of it.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{
    HostDeliveryLedgerPort, HostDeliveryPage, HostDeliveryQuery, HostDeliveryTargetFilter,
};
use made_core::value_objects::{HostDeliveryItemKind, HostDeliveryTarget};

use super::ListAttentionDeliveriesInput;

/// Reads the ledger the loop runs on.
///
/// The question this answers is the one a queue cannot: not "what is
/// waiting" but "what happened to the thing nobody ever came back
/// about". Endings stay in the ledger with their cause, so an offer
/// that expired, was shed by a full queue or was superseded by a
/// replacement reads differently from one still waiting — which is the
/// whole reason the loop is built on a ledger and not on a queue.
pub struct ListAttentionDeliveriesUseCase {
    deliveries: Arc<dyn HostDeliveryLedgerPort>,
}

impl std::fmt::Debug for ListAttentionDeliveriesUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ListAttentionDeliveriesUseCase")
            .finish_non_exhaustive()
    }
}

impl ListAttentionDeliveriesUseCase {
    #[must_use]
    pub fn new(deliveries: Arc<dyn HostDeliveryLedgerPort>) -> Self {
        Self { deliveries }
    }

    #[tracing::instrument(name = "list_attention_deliveries", skip_all)]
    pub async fn execute(
        &self,
        input: ListAttentionDeliveriesInput,
    ) -> Result<HostDeliveryPage, DomainError> {
        let mut query = HostDeliveryQuery::new().of_size(input.limit);
        if let Some(binding_id) = input.binding_id {
            query = query.to(HostDeliveryTargetFilter::any_of([
                HostDeliveryTarget::IntegratorBinding { binding_id },
            ])?);
        }
        if let Some(ceremony_id) = input.ceremony_id {
            query = query.in_ceremony(ceremony_id);
        }
        if let Some(state) = input.state {
            query = query.in_state(state);
        }
        if let Some(cursor) = input.cursor {
            query = query.after(cursor);
        }
        let page = self.deliveries.list(&query).await?;
        // Attention only. The same ledger carries interventions, and an
        // operator asking about the integrator loop being handed a
        // supervisor's question would read it as the loop's business.
        Ok(HostDeliveryPage::new(
            page.records()
                .iter()
                .filter(|record| record.item().kind() == HostDeliveryItemKind::Attention)
                .cloned()
                .collect(),
            page.next_cursor().cloned(),
        ))
    }
}

#[cfg(test)]
mod tests;
