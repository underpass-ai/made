//! [`InterventionDeliverySubscriber`] — a question sealed becomes a
//! question offered.
//!
//! # Why a subscriber and not a use case
//!
//! Every writer that can open an intervention would otherwise have to
//! remember to queue it, and the one that forgot would produce an item
//! nobody is ever offered and nobody can tell apart from one that was
//! offered and ignored. Being told by the stream means what is offered
//! is a function of what was sealed.
//!
//! # Nothing here can fail an append
//!
//! A ledger that cannot be written to is a delivery that has not
//! happened, not a ceremony that did not happen. Failures are logged
//! with the ceremony and the item, and the offer can be made again:
//! the delivery identity is derived from the item and its destination,
//! so enqueueing the same thing twice is one delivery.

use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{
    AgentExecutionStatus, AgentLiveness, AgentStatusSource, CeremonyAgentStatus, CeremonyEvent,
    CeremonyIntervention,
};
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyAgentStatusPort, CeremonyAgentStatusQuery, CeremonyEventSubscriberPort,
    HostDeliveryLedgerPort, PositionedRecord,
};
use made_core::value_objects::{
    CeremonyId, DeliveryExpiryCause, HostDeliveryItem, HostDeliveryPolicy, HostDeliveryRecord,
    HostDeliveryTarget, RoleId,
};
use time::OffsetDateTime;

/// Offers every intervention the stream opens to the hosts that can answer.
pub struct InterventionDeliverySubscriber {
    deliveries: Arc<dyn HostDeliveryLedgerPort>,
    statuses: Arc<dyn CeremonyAgentStatusPort>,
}

impl std::fmt::Debug for InterventionDeliverySubscriber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InterventionDeliverySubscriber")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl CeremonyEventSubscriberPort for InterventionDeliverySubscriber {
    async fn observe(&self, records: &[PositionedRecord]) {
        for positioned in records {
            let Some(event) = positioned.record.event() else {
                continue;
            };
            let ceremony_id = positioned.record.ceremony_id().clone();
            let at = positioned.record.occurred_at();
            if let Err(error) = self.react(&ceremony_id, event, at).await {
                tracing::warn!(
                    ceremony_id = %ceremony_id,
                    position = ?positioned.position,
                    %error,
                    "intervention delivery ledger write failed; the offer can be made again"
                );
            }
        }
    }
}

impl InterventionDeliverySubscriber {
    #[must_use]
    pub fn new(
        deliveries: Arc<dyn HostDeliveryLedgerPort>,
        statuses: Arc<dyn CeremonyAgentStatusPort>,
    ) -> Self {
        Self {
            deliveries,
            statuses,
        }
    }

    async fn react(
        &self,
        ceremony_id: &CeremonyId,
        event: &CeremonyEvent,
        at: OffsetDateTime,
    ) -> Result<(), DomainError> {
        match event {
            CeremonyEvent::InterventionRequested(requested) => {
                self.offer(ceremony_id, &requested.intervention, at).await
            }
            // A ceremony that has ended cannot be asked anything, so
            // the questions still in flight stop being worth making.
            // They stay visible as expired rather than disappearing:
            // "nobody was ever asked" is what an operator needs to be
            // able to read afterwards.
            CeremonyEvent::CeremonyCompleted(_)
            | CeremonyEvent::CeremonyCancelled(_)
            | CeremonyEvent::CeremonyDeadlineExceeded(_) => self.end_pending(ceremony_id, at).await,
            _ => Ok(()),
        }
    }

    /// Open one route per destination that could answer.
    ///
    /// An exact target is one route and only one: the question was put
    /// to that process generation. A role or the table gets a route per
    /// live execution of each role that may answer — so a working agent
    /// pulling its own questions finds it — plus one route addressed to
    /// the seat itself, which is what an agent that starts later, or a
    /// host the roster has never heard of, pulls against.
    async fn offer(
        &self,
        ceremony_id: &CeremonyId,
        intervention: &CeremonyIntervention,
        at: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let policy = intervention
            .delivery()
            .map_or_else(HostDeliveryPolicy::pull, |delivery| {
                delivery.host_policy().clone()
            });
        let item = HostDeliveryItem::intervention(ceremony_id.clone(), intervention.id().clone());
        for target in self.targets_of(ceremony_id, intervention).await? {
            let record = HostDeliveryRecord::queued(item.clone(), target, policy.clone(), at)?;
            self.deliveries.enqueue(record).await?;
        }
        Ok(())
    }

    async fn targets_of(
        &self,
        ceremony_id: &CeremonyId,
        intervention: &CeremonyIntervention,
    ) -> Result<Vec<HostDeliveryTarget>, DomainError> {
        if let Some(recipient) = intervention.target().exact_recipient() {
            return Ok(vec![recipient.exact_target()]);
        }
        let seats: Vec<RoleId> = match intervention.target().role_ids() {
            Some(role_ids) => role_ids.iter().cloned().collect(),
            None => self.roles_at_the_table(ceremony_id).await?,
        };
        let mut targets: Vec<HostDeliveryTarget> = seats
            .iter()
            .cloned()
            .map(HostDeliveryTarget::role)
            .collect();
        for status in self.live_agents(ceremony_id).await? {
            if seats.contains(status.role_id()) {
                targets.push(HostDeliveryTarget::agent_execution(
                    status.agent_execution_id().clone(),
                    status.host_agent_incarnation().clone(),
                ));
            }
        }
        Ok(targets)
    }

    /// The roles the roster has seen working, for a question put to
    /// the table rather than to named seats.
    async fn roles_at_the_table(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<Vec<RoleId>, DomainError> {
        Ok(self
            .live_agents(ceremony_id)
            .await?
            .into_iter()
            .map(|status| status.role_id().clone())
            .collect())
    }

    async fn live_agents(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<Vec<CeremonyAgentStatus>, DomainError> {
        let query = CeremonyAgentStatusQuery::new(ceremony_id.as_str(), None, 100, None)?;
        let page = self.statuses.list(query, None).await?;
        Ok(page
            .entries()
            .iter()
            .filter(|status| {
                // Only what a host said about itself, and only while
                // it is still reachable. A lease-derived row is the
                // engine's own inference and handing it a question
                // would be the engine talking to itself.
                status.source() == AgentStatusSource::HostReport
                    && status.liveness() != AgentLiveness::Unreachable
                    && status.execution_status() != AgentExecutionStatus::Finished
            })
            .cloned()
            .collect())
    }

    async fn end_pending(
        &self,
        ceremony_id: &CeremonyId,
        at: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let abandoned = self
            .deliveries
            .expire_ceremony(ceremony_id, DeliveryExpiryCause::CeremonyEnded, at)
            .await?;
        if !abandoned.is_empty() {
            tracing::info!(
                ceremony_id = %ceremony_id,
                abandoned = abandoned.len(),
                "ceremony ended with questions nobody had been handed"
            );
        }
        Ok(())
    }
}
