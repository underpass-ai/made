//! [`PullCeremonyAgentInterventionsUseCase`] — a working agent asks
//! what it has been asked.

use std::sync::Arc;

use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::ports::{
    ClockPort, HostDeliveryFilter, HostDeliveryLedgerPort, HostDeliveryTargetFilter,
};
use made_core::value_objects::{CeremonyInterventionId, HostDeliveryItem, HostDeliveryItemKind};

use super::ceremony_agent_status_service::CeremonyAgentStatusService;
use super::ceremony_intervention_view::CeremonyInterventionView;
use super::pull_ceremony_agent_interventions_input::PullCeremonyAgentInterventionsInput;
use super::pulled_ceremony_intervention::PulledCeremonyIntervention;
use super::pulled_ceremony_interventions::PulledCeremonyInterventions;
use crate::services::SessionStream;

pub struct PullCeremonyAgentInterventionsUseCase {
    stream: Arc<SessionStream>,
    statuses: Arc<CeremonyAgentStatusService>,
    deliveries: Arc<dyn HostDeliveryLedgerPort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for PullCeremonyAgentInterventionsUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PullCeremonyAgentInterventionsUseCase")
            .finish_non_exhaustive()
    }
}

impl PullCeremonyAgentInterventionsUseCase {
    #[must_use]
    pub fn new(
        stream: Arc<SessionStream>,
        statuses: Arc<CeremonyAgentStatusService>,
        deliveries: Arc<dyn HostDeliveryLedgerPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            stream,
            statuses,
            deliveries,
            clock,
        }
    }

    #[tracing::instrument(
        name = "pull_ceremony_agent_interventions",
        skip_all,
        fields(
            ceremony_id = %input.instance_id,
            agent_execution_id = %input.recipient.agent_execution_id(),
        )
    )]
    pub async fn execute(
        &self,
        input: PullCeremonyAgentInterventionsInput,
    ) -> Result<PulledCeremonyInterventions, DomainError> {
        // The claim is checked first and against the journal, so a
        // process whose lease has gone is told so before it is handed
        // anything. A question taken by an agent that is no longer
        // working is a question nobody will answer and nobody will
        // notice is unanswered.
        self.statuses
            .verify_live_recipient(&input.instance_id, &input.recipient)
            .await?;
        let session = self.stream.load(&input.instance_id).await?;
        let now = self.clock.now();

        // Expire before leasing: an offer whose holder never came back
        // is offerable again, and doing this on the read path means a
        // deployment with no background sweeper still recovers.
        self.deliveries.expire(now).await?;

        let filter = HostDeliveryFilter::to(HostDeliveryTargetFilter::any_of([
            input.recipient.exact_target(),
            input.recipient.role_target(),
        ])?)
        .in_ceremony(input.instance_id.clone())
        .of_kind(HostDeliveryItemKind::Intervention);
        let leased = self
            .deliveries
            .lease(
                &filter,
                input.recipient.incarnation(),
                now,
                input.lease_duration,
                input.limit,
            )
            .await?;

        let mut items = Vec::with_capacity(leased.len());
        for delivery in leased {
            let (lease, record) = delivery.into_parts();
            let HostDeliveryItem::Intervention {
                intervention_id, ..
            } = record.item()
            else {
                continue;
            };
            let Some(view) = self.view_of(&session.instance, intervention_id, &record) else {
                // The offer names an item this stream does not hold, or
                // holds closed. Hand the lease back rather than hand a
                // host a question that no longer exists.
                self.deliveries.release(&lease, now).await?;
                continue;
            };
            items.push(PulledCeremonyIntervention::new(lease, view));
        }
        Ok(PulledCeremonyInterventions::new(items))
    }

    fn view_of(
        &self,
        instance: &CeremonyInstance,
        intervention_id: &CeremonyInterventionId,
        record: &made_core::value_objects::HostDeliveryRecord,
    ) -> Option<CeremonyInterventionView> {
        let intervention = instance.intervention(intervention_id)?;
        if !intervention.status().is_open() {
            return None;
        }
        Some(CeremonyInterventionView::project(
            intervention.clone(),
            std::slice::from_ref(record),
        ))
    }
}
