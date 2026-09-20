use made_app::usecases::RequestCeremonyInterventionInput;
use made_core::value_objects::{
    AuditActorId, AuditActorKind, CeremonyAgentExecutionId, CeremonyId,
    CeremonyInterventionContent, CeremonyInterventionId, CeremonyInterventionIntent,
    CeremonyInterventionKind, CeremonyInterventionProvenance, CeremonyInterventionTarget,
    DeliveryAttemptLimit, DeliveryRecipient, DurationMs, FollowReplacement, HostAgentIncarnation,
    HostDeliveryMode, HostDeliveryPolicy, InterventionDeliveryPolicy, RoleId,
    SupervisorDisplayName, SupervisorPrincipal,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;
use uuid::Uuid;

use super::embedded_request_fields::{
    optional_attributes, optional_role_ids, optional_string, required_actor_kind, required_string,
};

use crate::protocol::ToolError;

/// Validated MCP request that opens a dynamic ceremony intervention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedRequestCeremonyInterventionRequest {
    ceremony_id: CeremonyId,
    intervention_id: CeremonyInterventionId,
    role_id: RoleId,
    role_kind: AuditActorKind,
    kind: CeremonyInterventionKind,
    target: CeremonyInterventionTarget,
    content: CeremonyInterventionContent,
    provenance: Option<CeremonyInterventionProvenance>,
    intent: Option<CeremonyInterventionIntent>,
    delivery: Option<InterventionDeliveryPolicy>,
    supervisor: Option<SupervisorPrincipal>,
}

impl EmbeddedRequestCeremonyInterventionRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<CeremonyId, ToolError> {
        let mut input = RequestCeremonyInterventionInput::new(
            self.ceremony_id.clone(),
            self.intervention_id,
            self.role_id,
            self.role_kind,
            self.kind,
            self.target,
            self.content,
        );
        if let Some(provenance) = self.provenance {
            input = input.with_provenance(provenance);
        }
        if let Some(intent) = self.intent {
            input = input.with_intent(intent);
        }
        if let Some(delivery) = self.delivery {
            input = input.with_delivery(delivery);
        }
        if let Some(supervisor) = self.supervisor {
            input = input.asked_by_supervisor(supervisor)?;
        }
        made.request_intervention(input).await?;
        Ok(self.ceremony_id)
    }
}

impl TryFrom<&Value> for EmbeddedRequestCeremonyInterventionRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let intervention_id = optional_string(object, "intervention_id")?
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let role_id =
            RoleId::new(required_string(object, "role_id")?).map_err(|error| error.to_string())?;
        // Naming one live agent is the most specific thing a caller can
        // say, so it wins over the seats. Both halves or neither: an
        // execution without its generation names a name rather than a
        // process, and addressing a process is the whole point.
        let exact = match (
            optional_string(object, "target_agent_execution_id")?,
            optional_string(object, "target_incarnation")?,
            optional_string(object, "target_role_id")?,
        ) {
            (Some(execution), Some(incarnation), Some(target_role)) => {
                Some(DeliveryRecipient::new(
                    CeremonyAgentExecutionId::new(execution).map_err(|error| error.to_string())?,
                    HostAgentIncarnation::new(incarnation).map_err(|error| error.to_string())?,
                    RoleId::new(target_role).map_err(|error| error.to_string())?,
                ))
            }
            (None, None, None) => None,
            _ => {
                return Err(
                    "an exact intervention target needs `target_agent_execution_id`, \
                     `target_incarnation` and `target_role_id`"
                        .to_owned(),
                )
            }
        };
        let target = match exact {
            Some(recipient) => Ok(CeremonyInterventionTarget::agent_execution(recipient)),
            None => match optional_role_ids(object, "target_role_ids")? {
                Some(role_ids) if !role_ids.is_empty() => {
                    CeremonyInterventionTarget::roles(role_ids)
                }
                _ => Ok(CeremonyInterventionTarget::table()),
            },
        }
        .map_err(|error| error.to_string())?;

        Ok(Self {
            ceremony_id: CeremonyId::new(required_string(object, "ceremony_id")?)
                .map_err(|error| error.to_string())?,
            intervention_id: CeremonyInterventionId::new(intervention_id)
                .map_err(|error| error.to_string())?,
            role_kind: required_actor_kind(object, "role_kind")?,
            role_id,
            kind: parse_kind(&required_string(object, "kind")?)?,
            target,
            content: CeremonyInterventionContent::new(
                required_string(object, "message")?,
                optional_attributes(object, "details")?,
            )
            .map_err(|error| error.to_string())?,
            provenance: optional_provenance(object)?,
            intent: optional_string(object, "intent")?
                .map(|intent| parse_intent(&intent))
                .transpose()?,
            delivery: optional_delivery(object)?,
            supervisor: optional_supervisor(object)?,
        })
    }
}

fn optional_provenance(
    object: &serde_json::Map<String, Value>,
) -> Result<Option<CeremonyInterventionProvenance>, String> {
    let Some(value) = object.get("provenance") else {
        return Ok(None);
    };
    let provenance = value
        .as_object()
        .ok_or_else(|| "field `provenance` must be an object".to_owned())?;
    Ok(Some(CeremonyInterventionProvenance::selected_from(
        CeremonyInterventionId::new(required_string(provenance, "source_intervention_id")?)
            .map_err(|error| error.to_string())?,
        RoleId::new(required_string(provenance, "source_response_role_id")?)
            .map_err(|error| error.to_string())?,
        RoleId::new(required_string(provenance, "selected_role_id")?)
            .map_err(|error| error.to_string())?,
    )))
}

fn parse_kind(raw: &str) -> Result<CeremonyInterventionKind, String> {
    match raw {
        "opinion" => Ok(CeremonyInterventionKind::Opinion),
        "investigation" => Ok(CeremonyInterventionKind::Investigation),
        "action" => Ok(CeremonyInterventionKind::Action),
        _ => Err("field `kind` must be one of: opinion, investigation, action".to_owned()),
    }
}

fn parse_intent(raw: &str) -> Result<CeremonyInterventionIntent, String> {
    match raw {
        "question" => Ok(CeremonyInterventionIntent::Question),
        "feedback" => Ok(CeremonyInterventionIntent::Feedback),
        "constraint" => Ok(CeremonyInterventionIntent::Constraint),
        "checkpoint" => Ok(CeremonyInterventionIntent::Checkpoint),
        _ => Err(
            "field `intent` must be one of: question, feedback, constraint, checkpoint".to_owned(),
        ),
    }
}

/// The terms the asker chose, with the engine's defaults under them.
fn optional_delivery(
    object: &serde_json::Map<String, Value>,
) -> Result<Option<InterventionDeliveryPolicy>, String> {
    let Some(value) = object.get("delivery") else {
        return Ok(None);
    };
    let delivery = value
        .as_object()
        .ok_or_else(|| "field `delivery` must be an object".to_owned())?;
    let mode = match delivery.get("mode").and_then(Value::as_str) {
        Some("activation") => HostDeliveryMode::Activation,
        _ => HostDeliveryMode::PullLease,
    };
    let lease = delivery
        .get("lease_duration_ms")
        .and_then(Value::as_u64)
        .unwrap_or(60_000);
    let ack_timeout = delivery
        .get("ack_timeout_ms")
        .and_then(Value::as_u64)
        .map(DurationMs::from_millis);
    let attempts = delivery
        .get("max_attempts")
        .and_then(Value::as_u64)
        .map(|value| u32::try_from(value).unwrap_or(u32::MAX))
        .map(DeliveryAttemptLimit::new)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let follow = if delivery
        .get("follow_replacement")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        FollowReplacement::Follow
    } else {
        FollowReplacement::Stay
    };
    Ok(Some(InterventionDeliveryPolicy::new(
        HostDeliveryPolicy::new(
            mode,
            DurationMs::from_millis(lease),
            ack_timeout,
            attempts,
            follow,
        )
        .map_err(|error| error.to_string())?,
    )))
}

fn optional_supervisor(
    object: &serde_json::Map<String, Value>,
) -> Result<Option<SupervisorPrincipal>, String> {
    let Some(value) = object.get("supervisor") else {
        return Ok(None);
    };
    let supervisor = value
        .as_object()
        .ok_or_else(|| "field `supervisor` must be an object".to_owned())?;
    Ok(Some(SupervisorPrincipal::new(
        AuditActorId::new(required_string(supervisor, "principal_id")?),
        SupervisorDisplayName::new(required_string(supervisor, "display")?)
            .map_err(|error| error.to_string())?,
    )))
}
