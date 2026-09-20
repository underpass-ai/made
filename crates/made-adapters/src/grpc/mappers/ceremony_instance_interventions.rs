//! One intervention, rendered for the versioned instance projection.
//!
//! Split out of the instance mapper because an item now carries its
//! routing, its intent and what hosts said about it, and a projection
//! that grew all of that inline would be one file holding two
//! contracts.

use made_core::entities::CeremonyIntervention;
use made_core::value_objects::{CeremonyInterventionResponse, RoleId};
use made_proto::v1 as pb;

use super::attributes::attributes_to_struct;
use super::ceremony_instance::moment;

pub(super) fn intervention_state_from(
    intervention: &CeremonyIntervention,
) -> pb::CeremonyInterventionState {
    pb::CeremonyInterventionState {
        intervention_id: intervention.id().as_str().to_owned(),
        kind: intervention.kind().as_label().to_owned(),
        status: intervention.status().as_label().to_owned(),
        requested_by: intervention.requested_by().as_str().to_owned(),
        target: Some(pb::CeremonyInterventionTargetState {
            kind: intervention.target().kind_str().to_owned(),
            role_ids: intervention
                .target()
                .role_ids()
                .map(|role_ids| {
                    role_ids
                        .iter()
                        .map(RoleId::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            agent_execution_id: intervention
                .target()
                .exact_recipient()
                .map(|recipient| recipient.agent_execution_id().as_str().to_owned())
                .unwrap_or_default(),
            incarnation: intervention
                .target()
                .exact_recipient()
                .map(|recipient| recipient.incarnation().as_str().to_owned())
                .unwrap_or_default(),
            role_id: intervention
                .target()
                .exact_recipient()
                .map(|recipient| recipient.role_id().as_str().to_owned())
                .unwrap_or_default(),
        }),
        request: Some(pb::CeremonyInterventionMessage {
            message: intervention.request().message().to_owned(),
            details: Some(attributes_to_struct(intervention.request().details())),
        }),
        provenance: intervention.provenance().map(|provenance| {
            pb::CeremonyInterventionProvenanceState {
                source_intervention_id: provenance.source_intervention_id().as_str().to_owned(),
                source_response_role_id: provenance.source_response_role_id().as_str().to_owned(),
                selected_role_id: provenance.selected_role_id().as_str().to_owned(),
            }
        }),
        responses: intervention
            .responses()
            .iter()
            .map(intervention_response_state_from)
            .collect(),
        created_at: moment(intervention.created_at()),
        updated_at: moment(intervention.updated_at()),
        closed_at: intervention.closed_at().map(moment).unwrap_or_default(),
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
        deliveries: intervention
            .deliveries()
            .iter()
            .map(|ack| pb::CeremonyInterventionDeliveryAckState {
                delivery_id: ack.delivery_id().to_string(),
                agent_execution_id: ack.recipient().agent_execution_id().as_str().to_owned(),
                incarnation: ack.recipient().incarnation().as_str().to_owned(),
                role_id: ack.recipient().role_id().as_str().to_owned(),
                observation_kind: ack.observation().kind().as_str().to_owned(),
                observation_note: ack.observation().note().as_str().to_owned(),
                acknowledged_at: moment(ack.acknowledged_at()),
            })
            .collect(),
    }
}

fn intervention_response_state_from(
    response: &CeremonyInterventionResponse,
) -> pb::CeremonyInterventionResponseState {
    pb::CeremonyInterventionResponseState {
        role_id: response.role_id().as_str().to_owned(),
        content: Some(pb::CeremonyInterventionMessage {
            message: response.content().message().to_owned(),
            details: Some(attributes_to_struct(response.content().details())),
        }),
        evidence_pack: response
            .evidence_pack()
            .map(|pack| serde_json::to_string(pack).unwrap_or_default())
            .unwrap_or_default(),
        responded_at: moment(response.responded_at()),
        executor_agent_execution_id: response
            .executor()
            .map(|executor| executor.agent_execution_id().as_str().to_owned())
            .unwrap_or_default(),
        executor_incarnation: response
            .executor()
            .map(|executor| executor.incarnation().as_str().to_owned())
            .unwrap_or_default(),
        delivery_id: response
            .delivery_id()
            .map(ToString::to_string)
            .unwrap_or_default(),
    }
}
