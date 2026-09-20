use made_core::entities::CeremonyIntervention;
use made_core::value_objects::{
    CeremonyInterventionStatus, DeliveryFailureReason, HostDeliveryRecord, HostDeliveryState,
    HostDeliveryStateKind,
};

use super::ceremony_intervention_delivery_status::CeremonyInterventionDeliveryStatus;
use super::delivery_route_view::DeliveryRouteView;

/// One intervention, with every route it took and where it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyInterventionView {
    intervention: CeremonyIntervention,
    routes: Vec<DeliveryRouteView>,
    status: CeremonyInterventionDeliveryStatus,
}

impl CeremonyInterventionView {
    /// Project one item from the stream and the ledger together.
    ///
    /// The stream wins wherever it speaks, because it is the sealed
    /// record of what the ceremony decided: an answered item is
    /// answered whatever the ledger still has queued. Below that, the
    /// furthest any route got is the answer, and the ladder only ever
    /// climbs on evidence — a lease or a receipt for `Delivered`, a
    /// sealed acknowledgement for `Acknowledged`. An item with routes
    /// that all ended badly reports the ending rather than quietly
    /// sitting at `Queued` forever.
    #[must_use]
    pub fn project(intervention: CeremonyIntervention, records: &[HostDeliveryRecord]) -> Self {
        let routes = records.iter().map(DeliveryRouteView::of).collect();
        let status = Self::status_of(&intervention, records);
        Self {
            intervention,
            routes,
            status,
        }
    }

    fn status_of(
        intervention: &CeremonyIntervention,
        records: &[HostDeliveryRecord],
    ) -> CeremonyInterventionDeliveryStatus {
        if intervention.status() == CeremonyInterventionStatus::Closed {
            return CeremonyInterventionDeliveryStatus::Closed;
        }
        if !intervention.responses().is_empty() {
            return CeremonyInterventionDeliveryStatus::Responded;
        }
        if records.is_empty() {
            // Nothing was ever offered. The item is real and sealed,
            // and saying "queued" about a queue it is not in would be
            // the first lie in the chain.
            return CeremonyInterventionDeliveryStatus::Recorded;
        }
        if intervention
            .deliveries()
            .iter()
            .any(|ack| ack.observation().kind().is_received())
        {
            return CeremonyInterventionDeliveryStatus::Acknowledged;
        }
        if records
            .iter()
            .any(|record| record.state().kind() == HostDeliveryStateKind::Queued)
        {
            return CeremonyInterventionDeliveryStatus::Queued;
        }
        if records
            .iter()
            .any(|record| Self::is_in_host_hands(record.state()))
        {
            return CeremonyInterventionDeliveryStatus::Delivered;
        }
        Self::ended_status(intervention, records)
    }

    /// Whether a host has been handed this route, on the engine's own
    /// evidence: an unexpired lease it took, or an activation receipt.
    fn is_in_host_hands(state: &HostDeliveryState) -> bool {
        matches!(
            state.kind(),
            HostDeliveryStateKind::Leased
                | HostDeliveryStateKind::DeliveredToHost
                | HostDeliveryStateKind::Acknowledged
                | HostDeliveryStateKind::Processed
        )
    }

    fn ended_status(
        intervention: &CeremonyIntervention,
        records: &[HostDeliveryRecord],
    ) -> CeremonyInterventionDeliveryStatus {
        // A host that took the item and said no has ended this route.
        // Reporting that as `Acknowledged` would leave an operator
        // waiting for an answer that was already declined.
        if let Some(refusal) = intervention
            .deliveries()
            .iter()
            .find(|ack| !ack.observation().kind().is_received())
        {
            if let Ok(reason) = DeliveryFailureReason::new(format!(
                "host observed {}: {}",
                refusal.observation().kind(),
                refusal.observation().note()
            )) {
                return CeremonyInterventionDeliveryStatus::Failed(reason);
            }
        }
        for record in records {
            match record.state() {
                HostDeliveryState::Failed { reason, .. } => {
                    return CeremonyInterventionDeliveryStatus::Failed(reason.clone());
                }
                HostDeliveryState::Expired { cause, .. } => {
                    return CeremonyInterventionDeliveryStatus::Expired(*cause);
                }
                _ => {}
            }
        }
        // Every route was superseded and none was followed: the
        // destinations this was addressed to are all gone.
        CeremonyInterventionDeliveryStatus::Unsupported
    }

    #[must_use]
    pub const fn intervention(&self) -> &CeremonyIntervention {
        &self.intervention
    }

    #[must_use]
    pub fn routes(&self) -> &[DeliveryRouteView] {
        &self.routes
    }

    #[must_use]
    pub const fn status(&self) -> &CeremonyInterventionDeliveryStatus {
        &self.status
    }

    /// Whether this item is still waiting on somebody.
    #[must_use]
    pub const fn is_unresolved(&self) -> bool {
        self.status.is_unresolved()
    }
}

#[cfg(test)]
mod tests {
    use made_core::value_objects::{
        Attributes, CeremonyAgentExecutionId, CeremonyId, CeremonyInterventionContent,
        CeremonyInterventionId, CeremonyInterventionKind, CeremonyInterventionTarget,
        DeliveryExpiryCause, DeliveryNote, DeliveryRecipient, HostAgentIncarnation,
        HostDeliveryItem, HostDeliveryLease, HostDeliveryLeaseId, HostDeliveryObservation,
        HostDeliveryObservationKind, HostDeliveryPolicy, HostDeliveryTarget,
        InterventionDeliveryAck, RoleId,
    };
    use time::macros::datetime;
    use time::OffsetDateTime;

    use super::*;

    fn at() -> OffsetDateTime {
        datetime!(2026-09-20 10:00:00 UTC)
    }

    fn recipient() -> DeliveryRecipient {
        DeliveryRecipient::new(
            CeremonyAgentExecutionId::new("exec-1").unwrap(),
            HostAgentIncarnation::new("inc-1").unwrap(),
            RoleId::new("ENGINEER").unwrap(),
        )
    }

    fn intervention() -> CeremonyIntervention {
        CeremonyIntervention::open(
            CeremonyInterventionId::new("item-1").unwrap(),
            CeremonyInterventionKind::Opinion,
            RoleId::new("LEAD").unwrap(),
            CeremonyInterventionTarget::agent_execution(recipient()),
            CeremonyInterventionContent::new("Still on plan?", Attributes::empty()).unwrap(),
            at(),
        )
    }

    fn queued_record() -> HostDeliveryRecord {
        HostDeliveryRecord::queued(
            HostDeliveryItem::intervention(
                CeremonyId::new("c-1").unwrap(),
                CeremonyInterventionId::new("item-1").unwrap(),
            ),
            recipient().exact_target(),
            HostDeliveryPolicy::pull(),
            at(),
        )
        .unwrap()
    }

    fn lease(record: &HostDeliveryRecord) -> HostDeliveryLease {
        HostDeliveryLease::new(
            record.id().clone(),
            HostDeliveryLeaseId::new("11111111-1111-4111-8111-111111111111").unwrap(),
            HostAgentIncarnation::new("inc-1").unwrap(),
            at() + time::Duration::seconds(60),
        )
    }

    #[test]
    fn an_item_nobody_ever_offered_is_recorded_and_not_queued() {
        let view = CeremonyInterventionView::project(intervention(), &[]);
        assert_eq!(view.status(), &CeremonyInterventionDeliveryStatus::Recorded);
        assert!(view.routes().is_empty());
        assert!(view.is_unresolved());
    }

    #[test]
    fn a_queued_route_never_reports_as_delivered() {
        let view = CeremonyInterventionView::project(intervention(), &[queued_record()]);
        assert_eq!(view.status(), &CeremonyInterventionDeliveryStatus::Queued);
        assert!(!view.status().has_reached_a_host());
    }

    #[test]
    fn a_lease_is_what_makes_delivered_true() {
        let record = queued_record();
        let leased = record.leased(lease(&record), at());
        let view = CeremonyInterventionView::project(intervention(), &[leased]);
        assert_eq!(
            view.status(),
            &CeremonyInterventionDeliveryStatus::Delivered
        );
    }

    #[test]
    fn only_a_sealed_acknowledgement_climbs_past_delivered() {
        let record = queued_record();
        let observation = HostDeliveryObservation::new(
            HostDeliveryObservationKind::Received,
            at(),
            None,
            DeliveryNote::new("taken").unwrap(),
        );
        // The ledger alone is not enough: the stream carries the fact.
        let acknowledged = record.acknowledged(observation.clone(), at());
        let mut item = intervention();
        assert_eq!(
            CeremonyInterventionView::project(item.clone(), &[acknowledged.clone()]).status(),
            &CeremonyInterventionDeliveryStatus::Delivered
        );
        item.acknowledge_delivery(InterventionDeliveryAck::new(
            record.id().clone(),
            recipient(),
            observation,
            at(),
        ))
        .unwrap();
        assert_eq!(
            CeremonyInterventionView::project(item, &[acknowledged]).status(),
            &CeremonyInterventionDeliveryStatus::Acknowledged
        );
    }

    #[test]
    fn an_item_whose_only_route_expired_says_so() {
        let expired = queued_record().expired(DeliveryExpiryCause::CeremonyEnded, at());
        let view = CeremonyInterventionView::project(intervention(), &[expired]);
        assert_eq!(
            view.status(),
            &CeremonyInterventionDeliveryStatus::Expired(DeliveryExpiryCause::CeremonyEnded)
        );
        assert!(!view.is_unresolved());
    }

    #[test]
    fn a_target_that_is_gone_is_unsupported_rather_than_waiting() {
        let superseded = queued_record().superseded(None, at());
        let view = CeremonyInterventionView::project(intervention(), &[superseded]);
        assert_eq!(
            view.status(),
            &CeremonyInterventionDeliveryStatus::Unsupported
        );
    }
}
