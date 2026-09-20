//! What an intervention and its routes look like on the wire.
//!
//! Its own file because the same shape is produced by four tools and by
//! both backends, and a presenter that lived inside one dispatcher
//! would be copied into the next one and then drift from it.

use made_app::usecases::{CeremonyInterventionView, DeliveryRouteView, PulledCeremonyIntervention};
use made_core::value_objects::{
    CeremonyInterventionIntent, CeremonyInterventionTarget, HostAgentIncarnation, HostDeliveryId,
    HostDeliveryLease, HostDeliveryTarget, InterventionDeliveryAck, RoleId,
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
        "intent": intervention.intent().map(CeremonyInterventionIntent::as_str),
        "target": present_target(intervention.target()),
        "provenance": Value::Null,
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
                "delivery_id": response.delivery_id().map(HostDeliveryId::as_str),
            }))
            .collect::<Vec<_>>(),
        "deliveries": intervention
            .deliveries()
            .iter()
            .map(present_ack)
            .collect::<Vec<_>>(),
        "created_at": rfc3339(intervention.created_at()),
        "updated_at": rfc3339(intervention.updated_at()),
        "closed_at": intervention.closed_at().map(rfc3339),
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

/// The same four fields the versioned contract carries, so a route
/// reads the same whichever engine answered.
fn present_delivery_target(route: &DeliveryRouteView) -> Value {
    let target = route.target();
    let key = target.target_key();
    json!({
        "kind": match target {
            HostDeliveryTarget::AgentExecution { .. } => "agent_execution",
            HostDeliveryTarget::Role { .. } => "role",
            HostDeliveryTarget::IntegratorBinding { .. } => "integrator_binding",
        },
        "agent_execution_id": key
            .as_str()
            .strip_prefix("agent:")
            .and_then(|rest| rest.split_once(':'))
            .map(|(execution, _)| execution),
        "incarnation": target.incarnation().map(HostAgentIncarnation::as_str),
        "role_id": target.role_id().map(RoleId::as_str),
    })
}

/// The same rule the instance projection follows: a target names what
/// it names, and the keys it does not use are absent rather than null.
fn present_target(target: &CeremonyInterventionTarget) -> Value {
    let mut value = json!({ "kind": target.kind_str() });
    let object = value
        .as_object_mut()
        .expect("a target renders as an object");
    if let Some(role_ids) = target.role_ids() {
        object.insert(
            "role_ids".to_owned(),
            json!(role_ids.iter().map(RoleId::as_str).collect::<Vec<_>>()),
        );
    }
    if let Some(recipient) = target.exact_recipient() {
        object.insert(
            "agent_execution_id".to_owned(),
            json!(recipient.agent_execution_id().as_str()),
        );
        object.insert(
            "incarnation".to_owned(),
            json!(recipient.incarnation().as_str()),
        );
        object.insert("role_id".to_owned(), json!(recipient.role_id().as_str()));
    }
    value
}
