//! [`ListProtocolInterventionsUseCase`] — what a protocol has been
//! asked, and what is still owed.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::HostDeliveryLedgerPort;

use super::list_protocol_interventions_input::ListProtocolInterventionsInput;
use super::protocol_intervention_page::ProtocolInterventionPage;
use super::protocol_intervention_routes::ProtocolInterventionRoutes;
use super::protocol_intervention_view::ProtocolInterventionView;
use crate::services::SessionStream;

pub struct ListProtocolInterventionsUseCase {
    stream: Arc<SessionStream>,
    deliveries: Arc<dyn HostDeliveryLedgerPort>,
}

impl std::fmt::Debug for ListProtocolInterventionsUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ListProtocolInterventionsUseCase")
            .finish_non_exhaustive()
    }
}

impl ListProtocolInterventionsUseCase {
    #[must_use]
    pub fn new(stream: Arc<SessionStream>, deliveries: Arc<dyn HostDeliveryLedgerPort>) -> Self {
        Self { stream, deliveries }
    }

    #[tracing::instrument(
        name = "list_protocol_interventions",
        skip_all,
        fields(protocol_id = %input.instance_id)
    )]
    pub async fn execute(
        &self,
        input: ListProtocolInterventionsInput,
    ) -> Result<ProtocolInterventionPage, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let routes = ProtocolInterventionRoutes::load(&self.deliveries, &input.instance_id).await?;

        // Keyset over the item id, which is the order the aggregate
        // holds them in, so a page boundary is stable while the
        // protocol keeps being asked things.
        let mut entries = Vec::new();
        let mut next_cursor = None;
        for intervention in session.instance.interventions() {
            if input
                .cursor
                .as_ref()
                .is_some_and(|cursor| intervention.id() <= cursor)
            {
                continue;
            }
            let view = ProtocolInterventionView::project(
                intervention.clone(),
                routes.of(intervention.id()),
            );
            if !Self::admits(&input, &view) {
                continue;
            }
            if entries.len() == input.limit.value() {
                next_cursor = entries
                    .last()
                    .map(|last: &ProtocolInterventionView| last.intervention().id().clone());
                break;
            }
            entries.push(view);
        }
        Ok(ProtocolInterventionPage::new(entries, next_cursor))
    }

    fn admits(input: &ListProtocolInterventionsInput, view: &ProtocolInterventionView) -> bool {
        if !input.resolution.admits_resolved() && !view.is_unresolved() {
            return false;
        }
        if let Some(status) = &input.status {
            // By name, so asking for "failed" finds every failure
            // rather than only one whose reason happens to match.
            if view.status().as_str() != status.as_str() {
                return false;
            }
        }
        if let Some(role_id) = &input.role_id {
            if !view.intervention().target().accepts(role_id) {
                return false;
            }
        }
        if let Some(agent_execution_id) = &input.agent_execution_id {
            let addressed = view
                .intervention()
                .target()
                .exact_recipient()
                .is_some_and(|recipient| recipient.agent_execution_id() == agent_execution_id);
            let routed = view.routes().iter().any(|route| {
                route
                    .target()
                    .target_key()
                    .as_str()
                    .starts_with(&format!("agent:{agent_execution_id}:"))
            });
            if !addressed && !routed {
                return false;
            }
        }
        true
    }
}
