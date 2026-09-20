//! What an intervention and its routes look like on the wire.
//!
//! Its own file because the same shape is produced by four tools and by
//! both backends, and a presenter that lived inside one dispatcher
//! would be copied into the next one and then drift from it.

use made_app::usecases::{
    CeremonyInterventionView, DeliveryRouteView, PulledCeremonyIntervention,
};
use made_core::entities::CeremonyIntervention;
use made_core::value_objects::{
    CeremonyInterventionTarget, HostDeliveryLease, InterventionDeliveryAck,
};
use serde_json::{json, Value};

use super::embedded_intervention_delivery_dispatch::rfc3339;

/// One leased offer, with the ticket needed to acknowledge it.
pub(super) fn present_lease(pulled: &PulledCeremonyIntervention) -> Value {
    json!({
        "delivery_id": pulled.lease().delivery_id().as_str(),
        "lease_id": pulled.lease().lease_id().as_str(),
        "leased_until": rfc3339(pulled.lease().leased_until()),
        "intervention": present_view(pulled.intervention()),
    })
}

/// One intervention, its routes, and where it stands.
pub(super) fn present_view(view: &CeremonyInterventionView) -> Value {
    let intervention = view.intervention();
    json!({
        "intervention_id": intervention.id().as_str(),
        "kind": intervention.kind().as_label(),
        "status": intervention.status().as_label(),
        "requested_by": intervention.requested_by().as_str(),
        "intent": intervention.intent().map(|intent| intent.as_str()),
        "target": present_target(intervention.target()),
        "message": intervention.request().message(),
        "supervisor": intervention.supervisor().map(|supervisor| {
            json!({
                "principal_id": supervisor.principal_id().as_str(),
                "display": supervisor.display().as_str(),
            })
        }),
        "responses": intervention
            .responses()
            .iter()
            .map(|response| json!({
                "role_id": response.role_id().as_str(),
                "message": response.content().message(),
                "responded_at": rfc3339(response.responded_at()),
                "executor_agent_execution_id": response
                    .executor()
                    .map(|executor| executor.agent_execution_id().as_str()),
                "executor_incarnation": response
                    .executor()
                    .map(|executor| executor.incarnation().as_str()),
                "delivery_id": response.delivery_id().map(|id| id.as_str()),
            }))
            .collect::<Vec<_>>(),
        "deliveries": intervention
            .deliveries()
            .iter()
            .map(present_ack)
            .collect::<Vec<_>>(),
        "routes": view.routes().iter().map(present_route).collect::<Vec<_>>(),
        // The projected status, and the reason it stopped when it did.
        // `delivered` here always has a lease or an activation receipt
        // behind it; a participant's own status label saying an
        // intervention was delivered is not an input to this.
        "status_delivery": view.status().as_str(),
        "status_reason": view.status().reason(),
        "unresolved": view.is_unresolved(),
    })
}

fn present_ack(ack: &InterventionDeliveryAck) -> Value {
    json!({
        "delivery_id": ack.delivery_id().as_str(),
        "agent_execution_id": ack.recipient().agent_execution_id().as_str(),
        "incarnation": ack.recipient().incarnation().as_str(),
        "role_id": ack.recipient().role_id().as_str(),
        "observation_kind": ack.observation().kind().as_str(),
        "observation_note": ack.observation().note().as_str(),
        "acknowledged_at": rfc3339(ack.acknowledged_at()),
    })
}

fn present_route(route: &DeliveryRouteView) -> Value {
    json!({
        "delivery_id": route.delivery_id().as_str(),
        "target": present_delivery_target(route),
        "state": route.state().as_str(),
        "attempt": route.attempt().value(),
        "lease_id": route.lease().map(|lease| lease.lease_id().as_str()),
        "leased_until": route.lease().map(|lease: &HostDeliveryLease| rfc3339(lease.leased_until())),
        "observation_kind": route
            .last_observation()
            .map(|observation| observation.kind().as_str()),
        "observation_note": route
            .last_observation()
            .map(|observation| observation.note().as_str()),
        "observed_at": route
            .last_observation()
            .map(|observation| rfc3339(observation.observed_at())),
    })
}

fn present_delivery_target(route: &DeliveryRouteView) -> Value {
    let target = route.target();
    json!({
        "key": target.target_key().as_str(),
        "role_id": target.role_id().map(|role| role.as_str()),
        "incarnation": target.incarnation().map(|incarnation| incarnation.as_str()),
        "binding_id": target.binding_id().map(|binding| binding.as_str()),
    })
}

fn present_target(target: &CeremonyInterventionTarget) -> Value {
    json!({
        "kind": target.kind_str(),
        "role_ids": target
            .role_ids()
            .map(|roles| roles.iter().map(|role| role.as_str()).collect::<Vec<_>>()),
        "agent_execution_id": target
            .exact_recipient()
            .map(|recipient| recipient.agent_execution_id().as_str()),
        "incarnation": target
            .exact_recipient()
            .map(|recipient| recipient.incarnation().as_str()),
        "role_id": target
            .exact_recipient()
            .map(|recipient| recipient.role_id().as_str()),
    })
}

/// The shape used where only the aggregate is to hand, with no ledger
/// behind it: routes are empty rather than invented.
pub(super) fn present_without_routes(intervention: &CeremonyIntervention) -> Value {
    present_view(&CeremonyInterventionView::project(
        intervention.clone(),
        &[],
    ))
}
