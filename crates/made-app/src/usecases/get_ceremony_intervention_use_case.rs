//! [`GetCeremonyInterventionUseCase`] — one item, and everywhere it went.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::HostDeliveryLedgerPort;

use super::ceremony_intervention_routes::CeremonyInterventionRoutes;
use super::ceremony_intervention_view::CeremonyInterventionView;
use super::get_ceremony_intervention_input::GetCeremonyInterventionInput;
use crate::services::SessionStream;

pub struct GetCeremonyInterventionUseCase {
    stream: Arc<SessionStream>,
    deliveries: Arc<dyn HostDeliveryLedgerPort>,
}

impl std::fmt::Debug for GetCeremonyInterventionUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GetCeremonyInterventionUseCase")
            .finish_non_exhaustive()
    }
}

impl GetCeremonyInterventionUseCase {
    #[must_use]
    pub fn new(stream: Arc<SessionStream>, deliveries: Arc<dyn HostDeliveryLedgerPort>) -> Self {
        Self { stream, deliveries }
    }

    #[tracing::instrument(
        name = "get_ceremony_intervention",
        skip_all,
        fields(
            ceremony_id = %input.instance_id,
            intervention_id = %input.intervention_id,
        )
    )]
    pub async fn execute(
        &self,
        input: GetCeremonyInterventionInput,
    ) -> Result<CeremonyInterventionView, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let intervention = session
            .instance
            .intervention(&input.intervention_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_intervention",
            })?
            .clone();
        let routes = CeremonyInterventionRoutes::load(&self.deliveries, &input.instance_id).await?;
        Ok(CeremonyInterventionView::project(
            intervention,
            routes.of(&input.intervention_id),
        ))
    }
}
