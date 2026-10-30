//! Saying why one thing a session produced led to another, over stdio.

use made_app::usecases::AssertProtocolReasonInput;
use made_core::value_objects::{
    GuardName, MemoryConfidence, ProtocolId, ProtocolInterventionId, ProtocolReasonKind,
    ProtocolRecordRef, RoleId, StepId,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::embedded_request_fields::{required_actor_kind, required_string};

use crate::mcp_protocol::ToolError;

pub(super) struct EmbeddedAssertProtocolReasonRequest {
    input: AssertProtocolReasonInput,
    protocol_id: ProtocolId,
}

impl EmbeddedAssertProtocolReasonRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<ProtocolId, ToolError> {
        made.assert_reason(self.input).await?;
        Ok(self.protocol_id)
    }
}

/// One end of a reason. Only the field the kind names is read.
fn record_ref(value: Option<&Value>, field: &str) -> Result<ProtocolRecordRef, String> {
    let object = value
        .and_then(Value::as_object)
        .ok_or_else(|| format!("`{field}` must be an object"))?;
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("`{field}.kind` is required"))?;
    let text = |key: &str| -> Result<String, String> {
        object
            .get(key)
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| format!("`{field}.{key}` is required for kind `{kind}`"))
    };
    let ordinal = || -> Result<u32, String> {
        object
            .get("ordinal")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| format!("`{field}.ordinal` is required for kind `{kind}`"))
    };

    Ok(match kind {
        "step" => ProtocolRecordRef::step(
            StepId::new(text("step_id")?).map_err(|error| stringify(&error))?,
        ),
        "agenda_item" => ProtocolRecordRef::agenda_item(
            ProtocolInterventionId::new(text("agenda_item")?).map_err(|error| stringify(&error))?,
        ),
        "contribution" => ProtocolRecordRef::contribution(
            ProtocolInterventionId::new(text("agenda_item")?).map_err(|error| stringify(&error))?,
            ordinal()?,
        ),
        "guard_decision" => ProtocolRecordRef::guard_decision(
            GuardName::new(text("guard_name")?).map_err(|error| stringify(&error))?,
        ),
        "transition" => ProtocolRecordRef::transition(ordinal()?),
        other => return Err(format!("`{field}.kind` does not name anything: {other}")),
    })
}

fn reason_kind(raw: &str) -> Result<ProtocolReasonKind, String> {
    Ok(match raw {
        "authorizes" => ProtocolReasonKind::Authorizes,
        "chosen_because" => ProtocolReasonKind::ChosenBecause,
        "achieved_by" => ProtocolReasonKind::AchievedBy,
        "follows_from" => ProtocolReasonKind::FollowsFrom,
        "satisfies_constraint" => ProtocolReasonKind::SatisfiesConstraint,
        "violates_constraint" => ProtocolReasonKind::ViolatesConstraint,
        "supersedes" => ProtocolReasonKind::Supersedes,
        "contradicts" => ProtocolReasonKind::Contradicts,
        "answers" => {
            return Err(
                "`answers` states the shape of the session rather than anyone's judgement, \
                 and only the engine asserts it"
                    .to_owned(),
            )
        }
        other => return Err(format!("`kind` does not name a reason: {other}")),
    })
}

fn confidence(raw: &str) -> Result<MemoryConfidence, String> {
    Ok(match raw {
        "high" => MemoryConfidence::High,
        "medium" => MemoryConfidence::Medium,
        "low" => MemoryConfidence::Low,
        other => {
            return Err(format!(
                "`confidence` must be high, medium or low, not {other}"
            ))
        }
    })
}

fn stringify(error: &made_core::error::DomainError) -> String {
    error.to_string()
}

impl TryFrom<&Value> for EmbeddedAssertProtocolReasonRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let protocol_id = ProtocolId::new(required_string(object, "protocol_id")?)
            .map_err(|error| stringify(&error))?;
        Ok(Self {
            input: AssertProtocolReasonInput::new(
                protocol_id.clone(),
                RoleId::new(required_string(object, "role_id")?)
                    .map_err(|error| stringify(&error))?,
                required_actor_kind(object, "role_kind")?,
                record_ref(object.get("from"), "from")?,
                record_ref(object.get("to"), "to")?,
                reason_kind(&required_string(object, "kind")?)?,
                required_string(object, "why")?,
                confidence(&required_string(object, "confidence")?)?,
            ),
            protocol_id,
        })
    }
}
