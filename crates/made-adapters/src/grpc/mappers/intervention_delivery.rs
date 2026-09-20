//! Between the wire and the use cases of intervention delivery.
//!
//! One direction reads what a host claims about itself and refuses what
//! it cannot; the other renders what the engine knows. They are in one
//! file because they are one contract, and splitting them would let the
//! two drift a field at a time.

use made_app::usecases::{
    AcknowledgeCeremonyAgentInterventionInput, CeremonyInterventionPage, CeremonyInterventionView,
    DeliveryRouteView, GetCeremonyInterventionInput, InterventionResolutionFilter,
    ListCeremonyInterventionsInput, PullCeremonyAgentInterventionsInput,
    PulledCeremonyIntervention, PulledCeremonyInterventions,
};
use made_core::error::DomainError;
use made_core::ports::HostDeliveryPageLimit;
use made_core::value_objects::{
    CeremonyAgentExecutionId, CeremonyId, CeremonyInterventionId, CeremonyInterventionPageLimit,
    CeremonyInterventionTarget, DeliveryNote, DeliveryRecipient, DurationMs, EvidenceReference,
    HostAgentIncarnation, HostDeliveryId, HostDeliveryLease, HostDeliveryLeaseId,
    HostDeliveryObservation, HostDeliveryObservationKind, InterventionDeliveryAck, RoleId,
};
use made_proto::v1 as pb;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub fn pull_ceremony_agent_interventions_input_from_proto(
    request: pb::PullCeremonyAgentInterventionsRequest,
) -> Result<PullCeremonyAgentInterventionsInput, DomainError> {
    let mut input = PullCeremonyAgentInterventionsInput::new(
        CeremonyId::new(request.ceremony_id)?,
        recipient(
            request.agent_execution_id,
            request.incarnation,
            request.role_id,
        )?,
    );
    if request.lease_duration_ms > 0 {
        input = input.leased_for(DurationMs::from_millis(request.lease_duration_ms))?;
    }
    if request.limit > 0 {
        input = input.of_size(HostDeliveryPageLimit::new(request.limit)?);
    }
    Ok(input)
}

pub fn acknowledge_ceremony_agent_intervention_input_from_proto(
    request: pb::AcknowledgeCeremonyAgentInterventionRequest,
) -> Result<AcknowledgeCeremonyAgentInterventionInput, DomainError> {
    let recipient = recipient(
        request.agent_execution_id,
        request.incarnation,
        request.role_id,
    )?;
    let kind = observation_kind(&request.observation_kind)?;
    let note = DeliveryNote::new(if request.note.is_empty() {
        format!("host observed {kind}")
    } else {
        request.note
    })?;
    let evidence = if request.evidence.is_empty() {
        None
    } else {
        Some(EvidenceReference::new(request.evidence)?)
    };
    let now = OffsetDateTime::now_utc();
    let observed_at = if request.observed_at.is_empty() {
        now
    } else {
        OffsetDateTime::parse(&request.observed_at, &Rfc3339).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "observed_at must be an RFC3339 instant",
            }
        })?
    };
    Ok(AcknowledgeCeremonyAgentInterventionInput::new(
        CeremonyId::new(request.ceremony_id)?,
        CeremonyInterventionId::new(request.intervention_id)?,
        HostDeliveryLease::new(
            HostDeliveryId::new(request.delivery_id)?,
            HostDeliveryLeaseId::new(request.lease_id)?,
            recipient.incarnation().clone(),
            now,
        ),
        recipient,
        HostDeliveryObservation::new(kind, observed_at, evidence, note),
    ))
}

pub fn get_ceremony_intervention_input_from_proto(
    request: pb::GetCeremonyInterventionRequest,
) -> Result<GetCeremonyInterventionInput, DomainError> {
    Ok(GetCeremonyInterventionInput::new(
        CeremonyId::new(request.ceremony_id)?,
        CeremonyInterventionId::new(request.intervention_id)?,
    ))
}

pub fn list_ceremony_interventions_input_from_proto(
    request: pb::ListCeremonyInterventionsRequest,
) -> Result<ListCeremonyInterventionsInput, DomainError> {
    let mut input = ListCeremonyInterventionsInput::new(CeremonyId::new(request.ceremony_id)?);
    if !request.status.is_empty() {
        input = input.in_status(&request.status)?;
    }
    if !request.role_id.is_empty() {
        input = input.for_role(RoleId::new(request.role_id)?);
    }
    if !request.agent_execution_id.is_empty() {
        input =
            input.for_agent_execution(CeremonyAgentExecutionId::new(request.agent_execution_id)?);
    }
    input = input.resolved(InterventionResolutionFilter::from_unresolved_only(
        request.unresolved_only,
    ));
    if request.limit > 0 {
        input = input.of_size(CeremonyInterventionPageLimit::new(request.limit as usize)?);
    }
    if !request.cursor.is_empty() {
        input = input.after(CeremonyInterventionId::new(request.cursor)?);
    }
    Ok(input)
}

#[must_use]
pub fn pulled_interventions_to_proto(
    pulled: &PulledCeremonyInterventions,
) -> Vec<pb::CeremonyAgentInterventionLeaseState> {
    pulled.items().iter().map(lease_state).collect()
}

fn lease_state(pulled: &PulledCeremonyIntervention) -> pb::CeremonyAgentInterventionLeaseState {
    pb::CeremonyAgentInterventionLeaseState {
        delivery: Some(intervention_delivery_state(pulled.intervention())),
        delivery_id: pulled.lease().delivery_id().to_string(),
        lease_id: pulled.lease().lease_id().as_str().to_owned(),
        leased_until: rfc3339(pulled.lease().leased_until()),
    }
}

#[must_use]
pub fn intervention_page_to_proto(
    page: &CeremonyInterventionPage,
) -> Vec<pb::CeremonyInterventionDeliveryState> {
    page.entries()
        .iter()
        .map(intervention_delivery_state)
        .collect()
}

#[must_use]
pub fn intervention_delivery_state(
    view: &CeremonyInterventionView,
) -> pb::CeremonyInterventionDeliveryState {
    pb::CeremonyInterventionDeliveryState {
        intervention: Some(intervention_state(view)),
        routes: view.routes().iter().map(route_state).collect(),
        status: view.status().as_str().to_owned(),
        status_reason: view.status().reason().unwrap_or_default(),
        unresolved: view.is_unresolved(),
    }
}

fn intervention_state(view: &CeremonyInterventionView) -> pb::CeremonyInterventionState {
    let intervention = view.intervention();
    pb::CeremonyInterventionState {
        intervention_id: intervention.id().as_str().to_owned(),
        kind: intervention.kind().as_label().to_owned(),
        status: intervention.status().as_label().to_owned(),
        requested_by: intervention.requested_by().as_str().to_owned(),
        target: Some(target_state(intervention.target())),
        request: Some(pb::CeremonyInterventionMessage {
            message: intervention.request().message().to_owned(),
            details: None,
        }),
        provenance: None,
        responses: Vec::new(),
        created_at: rfc3339(intervention.created_at()),
        updated_at: rfc3339(intervention.updated_at()),
        closed_at: intervention.closed_at().map(rfc3339).unwrap_or_default(),
        intent: intervention
            .intent()
            .map(|intent| intent.as_str().to_owned())
            .unwrap_or_default(),
        supervisor_principal_id: intervention
            .supervisor()
            .map(|supervisor| supervisor.principal_id().as_str().to_owned())
            .unwrap_or_default(),
        supervisor_display: intervention
            .supervisor()
            .map(|supervisor| supervisor.display().as_str().to_owned())
            .unwrap_or_default(),
        deliveries: intervention.deliveries().iter().map(ack_state).collect(),
    }
}

fn target_state(target: &CeremonyInterventionTarget) -> pb::CeremonyInterventionTargetState {
    pb::CeremonyInterventionTargetState {
        kind: target.kind_str().to_owned(),
        role_ids: target.role_ids().map_or_else(Vec::new, |roles| {
            roles.iter().map(|role| role.as_str().to_owned()).collect()
        }),
        agent_execution_id: target
            .exact_recipient()
            .map(|recipient| recipient.agent_execution_id().as_str().to_owned())
            .unwrap_or_default(),
        incarnation: target
            .exact_recipient()
            .map(|recipient| recipient.incarnation().as_str().to_owned())
            .unwrap_or_default(),
        role_id: target
            .exact_recipient()
            .map(|recipient| recipient.role_id().as_str().to_owned())
            .unwrap_or_default(),
    }
}

fn ack_state(ack: &InterventionDeliveryAck) -> pb::CeremonyInterventionDeliveryAckState {
    pb::CeremonyInterventionDeliveryAckState {
        delivery_id: ack.delivery_id().to_string(),
        agent_execution_id: ack.recipient().agent_execution_id().as_str().to_owned(),
        incarnation: ack.recipient().incarnation().as_str().to_owned(),
        role_id: ack.recipient().role_id().as_str().to_owned(),
        observation_kind: ack.observation().kind().as_str().to_owned(),
        observation_note: ack.observation().note().as_str().to_owned(),
        acknowledged_at: rfc3339(ack.acknowledged_at()),
    }
}

fn route_state(route: &DeliveryRouteView) -> pb::CeremonyInterventionDeliveryRouteState {
    let target = route.target();
    pb::CeremonyInterventionDeliveryRouteState {
        delivery_id: route.delivery_id().to_string(),
        target_kind: target_kind(target).to_owned(),
        target_agent_execution_id: target
            .target_key()
            .as_str()
            .strip_prefix("agent:")
            .and_then(|rest| rest.split_once(':'))
            .map(|(execution, _)| execution.to_owned())
            .unwrap_or_default(),
        target_incarnation: target
            .incarnation()
            .map(|incarnation| incarnation.as_str().to_owned())
            .unwrap_or_default(),
        target_role_id: target
            .role_id()
            .map(|role| role.as_str().to_owned())
            .unwrap_or_default(),
        state: route.state().as_str().to_owned(),
        attempt: route.attempt().value(),
        lease_id: route
            .lease()
            .map(|lease| lease.lease_id().as_str().to_owned())
            .unwrap_or_default(),
        leased_until: route
            .lease()
            .map(|lease| rfc3339(lease.leased_until()))
            .unwrap_or_default(),
        observation_kind: route
            .last_observation()
            .map(|observation| observation.kind().as_str().to_owned())
            .unwrap_or_default(),
        observation_note: route
            .last_observation()
            .map(|observation| observation.note().as_str().to_owned())
            .unwrap_or_default(),
        observed_at: route
            .last_observation()
            .map(|observation| rfc3339(observation.observed_at()))
            .unwrap_or_default(),
    }
}

fn target_kind(target: &made_core::value_objects::HostDeliveryTarget) -> &'static str {
    match target {
        made_core::value_objects::HostDeliveryTarget::AgentExecution { .. } => "agent_execution",
        made_core::value_objects::HostDeliveryTarget::Role { .. } => "role",
        made_core::value_objects::HostDeliveryTarget::IntegratorBinding { .. } => {
            "integrator_binding"
        }
    }
}

fn recipient(
    agent_execution_id: String,
    incarnation: String,
    role_id: String,
) -> Result<DeliveryRecipient, DomainError> {
    Ok(DeliveryRecipient::new(
        CeremonyAgentExecutionId::new(agent_execution_id)?,
        HostAgentIncarnation::new(incarnation)?,
        RoleId::new(role_id)?,
    ))
}

fn observation_kind(raw: &str) -> Result<HostDeliveryObservationKind, DomainError> {
    serde_json::from_value(serde_json::Value::String(raw.to_owned())).map_err(|_| {
        DomainError::InvariantViolated {
            reason: "observation kind must be received, refused, incapable, busy or timeout",
        }
    })
}

fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&Rfc3339).unwrap_or_else(|_| at.to_string())
}
