//! [`ListCeremonyInterventionsUseCase`] — what a ceremony has been
//! asked, and what is still owed.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::HostDeliveryLedgerPort;

use super::ceremony_intervention_page::CeremonyInterventionPage;
use super::ceremony_intervention_routes::CeremonyInterventionRoutes;
use super::ceremony_intervention_view::CeremonyInterventionView;
use super::list_ceremony_interventions_input::ListCeremonyInterventionsInput;
use crate::services::SessionStream;

pub struct ListCeremonyInterventionsUseCase {
    stream: Arc<SessionStream>,
    deliveries: Arc<dyn HostDeliveryLedgerPort>,
}

impl std::fmt::Debug for ListCeremonyInterventionsUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ListCeremonyInterventionsUseCase")
            .finish_non_exhaustive()
    }
}

impl ListCeremonyInterventionsUseCase {
    #[must_use]
    pub fn new(stream: Arc<SessionStream>, deliveries: Arc<dyn HostDeliveryLedgerPort>) -> Self {
        Self { stream, deliveries }
    }

    #[tracing::instrument(
        name = "list_ceremony_interventions",
        skip_all,
        fields(ceremony_id = %input.instance_id)
    )]
    pub async fn execute(
        &self,
        input: ListCeremonyInterventionsInput,
    ) -> Result<CeremonyInterventionPage, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let routes = CeremonyInterventionRoutes::load(&self.deliveries, &input.instance_id).await?;

        // Keyset over the item id, which is the order the aggregate
        // holds them in, so a page boundary is stable while the
        // ceremony keeps being asked things.
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
            let view = CeremonyInterventionView::project(
                intervention.clone(),
                routes.of(intervention.id()),
            );
            if !Self::admits(&input, &view) {
                continue;
            }
            if entries.len() == input.limit {
                next_cursor = entries
                    .last()
                    .map(|last: &CeremonyInterventionView| last.intervention().id().clone());
                break;
            }
            entries.push(view);
        }
        Ok(CeremonyInterventionPage::new(entries, next_cursor))
    }

    fn admits(input: &ListCeremonyInterventionsInput, view: &CeremonyInterventionView) -> bool {
        if input.unresolved_only && !view.is_unresolved() {
            return false;
        }
        if let Some(status) = &input.status {
            if view.status().as_str() != status {
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
