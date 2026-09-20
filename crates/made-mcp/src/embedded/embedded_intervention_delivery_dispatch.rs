use made_app::usecases::{
    AcknowledgeCeremonyAgentInterventionInput, GetCeremonyInterventionInput,
    ListCeremonyInterventionsInput, PullCeremonyAgentInterventionsInput,
};
use made_core::ports::HostDeliveryPageLimit;
use made_core::value_objects::{
    CeremonyAgentExecutionId, CeremonyId, CeremonyInterventionId, DeliveryNote, DeliveryRecipient,
    DurationMs, EvidenceReference, HostAgentIncarnation, HostDeliveryId, HostDeliveryLease,
    HostDeliveryLeaseId, HostDeliveryObservation, HostDeliveryObservationKind, RoleId,
};
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::embedded_intervention_delivery_presenter::{
    present_lease, present_view, present_without_routes,
};
use crate::protocol::{tool_success_result, ToolError};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_pull_ceremony_agent_interventions"
            | "made_acknowledge_ceremony_agent_intervention"
            | "made_get_ceremony_intervention"
            | "made_list_ceremony_interventions"
    )
}

pub(super) async fn dispatch(
    made: &EmbeddedMade,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError> {
    let object = arguments
        .as_object()
        .ok_or_else(|| ToolError::invalid_request("tools/call.arguments must be an object"))?;
    let ceremony = CeremonyId::new(required(object, "ceremony_id")?)?;
    let value = match name {
        "made_pull_ceremony_agent_interventions" => {
            let mut input =
                PullCeremonyAgentInterventionsInput::new(ceremony, recipient(object)?);
            if let Some(millis) = object.get("lease_duration_ms").and_then(Value::as_u64) {
                input = input.leased_for(DurationMs::from_millis(millis))?;
            }
            if let Some(limit) = object.get("limit").and_then(Value::as_u64) {
                input = input.of_size(HostDeliveryPageLimit::new(limit as u32)?);
            }
            let pulled = made.pull_agent_interventions(input).await?;
            json!({
                "items": pulled.items().iter().map(present_lease).collect::<Vec<_>>()
            })
        }
        "made_acknowledge_ceremony_agent_intervention" => {
            let recipient = recipient(object)?;
            let lease = HostDeliveryLease::new(
                HostDeliveryId::new(required(object, "delivery_id")?)?,
                HostDeliveryLeaseId::new(required(object, "lease_id")?)?,
                recipient.incarnation().clone(),
                // The ledger compares the lease identifier, not this
                // instant: a host repeating what it was handed does not
                // get to extend its own hold by saying so.
                OffsetDateTime::now_utc(),
            );
            let instance = made
                .acknowledge_agent_intervention(AcknowledgeCeremonyAgentInterventionInput::new(
                    ceremony,
                    CeremonyInterventionId::new(required(object, "intervention_id")?)?,
                    lease,
                    recipient,
                    observation(object)?,
                ))
                .await?;
            json!({
                "ceremony_id": instance.id().as_str(),
                "interventions": instance
                    .interventions()
                    .iter()
                    .map(present_without_routes)
                    .collect::<Vec<_>>()
            })
        }
        "made_get_ceremony_intervention" => {
            let view = made
                .get_intervention(GetCeremonyInterventionInput::new(
                    ceremony,
                    CeremonyInterventionId::new(required(object, "intervention_id")?)?,
                ))
                .await?;
            present_view(&view)
        }
        "made_list_ceremony_interventions" => {
            let mut input = ListCeremonyInterventionsInput::new(ceremony);
            if let Some(status) = optional(object, "status") {
                input = input.in_status(status);
            }
            if let Some(role_id) = optional(object, "role_id") {
                input = input.for_role(RoleId::new(role_id)?);
            }
            if let Some(execution) = optional(object, "agent_execution_id") {
                input = input.for_agent_execution(CeremonyAgentExecutionId::new(execution)?);
            }
            if let Some(unresolved) = object.get("unresolved_only").and_then(Value::as_bool) {
                input = input.unresolved_only(unresolved);
            }
            if let Some(limit) = object.get("limit").and_then(Value::as_u64) {
                input = input.of_size(limit as usize)?;
            }
            if let Some(cursor) = optional(object, "cursor") {
                input = input.after(CeremonyInterventionId::new(cursor)?);
            }
            let page = made.list_interventions(input).await?;
            json!({
                "interventions": page.entries().iter().map(present_view).collect::<Vec<_>>(),
                "next_cursor": page.next_cursor().map(CeremonyInterventionId::as_str)
            })
        }
        _ => unreachable!("the dispatcher only routes what it handles"),
    };
    Ok(tool_success_result(value))
}

fn recipient(
    object: &serde_json::Map<String, Value>,
) -> Result<DeliveryRecipient, ToolError> {
    Ok(DeliveryRecipient::new(
        CeremonyAgentExecutionId::new(required(object, "agent_execution_id")?)?,
        HostAgentIncarnation::new(required(object, "incarnation")?)?,
        RoleId::new(required(object, "role_id")?)?,
    ))
}

fn observation(
    object: &serde_json::Map<String, Value>,
) -> Result<HostDeliveryObservation, ToolError> {
    let kind: HostDeliveryObservationKind = serde_json::from_value(
        object
            .get("observation_kind")
            .cloned()
            .unwrap_or(Value::Null),
    )
    .map_err(|_| {
        ToolError::invalid_request(
            "observation_kind must be received, refused, incapable, busy or timeout",
        )
    })?;
    let note = DeliveryNote::new(
        optional(object, "note").unwrap_or_else(|| format!("host observed {kind}")),
    )?;
    let evidence = optional(object, "evidence")
        .map(EvidenceReference::new)
        .transpose()?;
    Ok(HostDeliveryObservation::new(
        kind,
        OffsetDateTime::now_utc(),
        evidence,
        note,
    ))
}

fn required<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, ToolError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ToolError::invalid_request(format!("missing required string `{key}`")))
}

fn optional(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// RFC3339, which is what every other timestamp on this surface is.
pub(super) fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&Rfc3339).unwrap_or_else(|_| at.to_string())
}
