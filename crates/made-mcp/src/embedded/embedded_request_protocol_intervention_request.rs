use made_app::usecases::RequestProtocolInterventionInput;
use made_core::value_objects::{
    AuditActorId, AuditActorKind, DeliveryAttemptLimit, DeliveryRecipient, DurationMs,
    FollowReplacement, HostAgentIncarnation, HostDeliveryMode, HostDeliveryPolicy,
    InterventionDeliveryPolicy, ProtocolAgentExecutionId, ProtocolId, ProtocolInterventionContent,
    ProtocolInterventionId, ProtocolInterventionIntent, ProtocolInterventionKind,
    ProtocolInterventionProvenance, ProtocolInterventionTarget, RoleId, SupervisorDisplayName,
    SupervisorPrincipal,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;
use uuid::Uuid;

use super::embedded_request_fields::{
    optional_attributes, optional_role_ids, optional_string, required_actor_kind, required_string,
};

use crate::mcp_protocol::ToolError;

/// Validated MCP request that opens a dynamic protocol intervention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedRequestProtocolInterventionRequest {
    protocol_id: ProtocolId,
    intervention_id: ProtocolInterventionId,
    role_id: RoleId,
    role_kind: AuditActorKind,
    kind: ProtocolInterventionKind,
    target: ProtocolInterventionTarget,
    content: ProtocolInterventionContent,
    provenance: Option<ProtocolInterventionProvenance>,
    intent: Option<ProtocolInterventionIntent>,
    delivery: Option<InterventionDeliveryPolicy>,
    supervisor: Option<SupervisorPrincipal>,
}

impl EmbeddedRequestProtocolInterventionRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<ProtocolId, ToolError> {
        let mut input = RequestProtocolInterventionInput::new(
            self.protocol_id.clone(),
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
        Ok(self.protocol_id)
    }
}

impl TryFrom<&Value> for EmbeddedRequestProtocolInterventionRequest {
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
                    ProtocolAgentExecutionId::new(execution).map_err(|error| error.to_string())?,
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
            Some(recipient) => Ok(ProtocolInterventionTarget::agent_execution(recipient)),
            None => match optional_role_ids(object, "target_role_ids")? {
                Some(role_ids) if !role_ids.is_empty() => {
                    ProtocolInterventionTarget::roles(role_ids)
                }
                _ => Ok(ProtocolInterventionTarget::table()),
            },
        }
        .map_err(|error| error.to_string())?;

        Ok(Self {
            protocol_id: ProtocolId::new(required_string(object, "protocol_id")?)
                .map_err(|error| error.to_string())?,
            intervention_id: ProtocolInterventionId::new(intervention_id)
                .map_err(|error| error.to_string())?,
            role_kind: required_actor_kind(object, "role_kind")?,
            role_id,
            kind: parse_kind(&required_string(object, "kind")?)?,
            target,
            content: ProtocolInterventionContent::new(
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
) -> Result<Option<ProtocolInterventionProvenance>, String> {
    let Some(value) = object.get("provenance") else {
        return Ok(None);
    };
    let provenance = value
        .as_object()
        .ok_or_else(|| "field `provenance` must be an object".to_owned())?;
    Ok(Some(ProtocolInterventionProvenance::selected_from(
        ProtocolInterventionId::new(required_string(provenance, "source_intervention_id")?)
            .map_err(|error| error.to_string())?,
        RoleId::new(required_string(provenance, "source_response_role_id")?)
            .map_err(|error| error.to_string())?,
        RoleId::new(required_string(provenance, "selected_role_id")?)
            .map_err(|error| error.to_string())?,
    )))
}

fn parse_kind(raw: &str) -> Result<ProtocolInterventionKind, String> {
    match raw {
        "opinion" => Ok(ProtocolInterventionKind::Opinion),
        "investigation" => Ok(ProtocolInterventionKind::Investigation),
        "action" => Ok(ProtocolInterventionKind::Action),
        _ => Err("field `kind` must be one of: opinion, investigation, action".to_owned()),
    }
}

fn parse_intent(raw: &str) -> Result<ProtocolInterventionIntent, String> {
    match raw {
        "question" => Ok(ProtocolInterventionIntent::Question),
        "feedback" => Ok(ProtocolInterventionIntent::Feedback),
        "constraint" => Ok(ProtocolInterventionIntent::Constraint),
        "checkpoint" => Ok(ProtocolInterventionIntent::Checkpoint),
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
