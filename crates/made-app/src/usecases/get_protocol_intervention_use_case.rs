//! [`GetProtocolInterventionUseCase`] — one item, and everywhere it went.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::HostDeliveryLedgerPort;

use super::get_protocol_intervention_input::GetProtocolInterventionInput;
use super::protocol_intervention_routes::ProtocolInterventionRoutes;
use super::protocol_intervention_view::ProtocolInterventionView;
use crate::services::SessionStream;

pub struct GetProtocolInterventionUseCase {
    stream: Arc<SessionStream>,
    deliveries: Arc<dyn HostDeliveryLedgerPort>,
}

impl std::fmt::Debug for GetProtocolInterventionUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GetProtocolInterventionUseCase")
            .finish_non_exhaustive()
    }
}

impl GetProtocolInterventionUseCase {
    #[must_use]
    pub fn new(stream: Arc<SessionStream>, deliveries: Arc<dyn HostDeliveryLedgerPort>) -> Self {
        Self { stream, deliveries }
    }

    #[tracing::instrument(
        name = "get_protocol_intervention",
        skip_all,
        fields(
            protocol_id = %input.instance_id,
            intervention_id = %input.intervention_id,
        )
    )]
    pub async fn execute(
        &self,
        input: GetProtocolInterventionInput,
    ) -> Result<ProtocolInterventionView, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        let intervention = session
            .instance
            .intervention(&input.intervention_id)
            .ok_or(DomainError::NotFound {
                what: "protocol_intervention",
            })?
            .clone();
        let routes = ProtocolInterventionRoutes::load(&self.deliveries, &input.instance_id).await?;
        Ok(ProtocolInterventionView::project(
            intervention,
            routes.of(&input.intervention_id),
        ))
    }
}
