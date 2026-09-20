//! Reading the nine agentic-system requests off the wire.
//!
//! The design document is handed straight to the application's own
//! decoder rather than picked apart here. One decoder is what makes
//! the same document produce the same digest on either backend; a
//! second reading of it would only be a second chance to disagree.

use std::collections::BTreeMap;

use made_app::usecases::agentic_system::{
    AgenticSystemDesignDocument, InstantiateAgenticSystemInput, ParticipantOffer,
};
use made_core::ports::AgenticSystemQuery;
use made_core::value_objects::{
    AgenticSystemExecutionId, AgenticSystemId, AgenticSystemLifecycle, AgenticSystemPageLimit,
    AgenticSystemRevision, Attributes, AuditActorId, AuditActorKind, Capability, CeremonyContext,
    HostActivationMode, HostAddress, HostDestination, HostKind, ParticipantId, Specialty,
    SystemCeremonyId,
};
use serde_json::Value;

use crate::protocol::ToolError;

/// The design document, decoded by the one decoder that owns it.
pub(super) fn design_document(arguments: &Value) -> Result<AgenticSystemDesignDocument, ToolError> {
    let design = arguments
        .get("design")
        .ok_or_else(|| ToolError::invalid_request("field `design` is required"))?;
    serde_json::from_value(design.clone())
        .map_err(|error| ToolError::invalid_request(format!("invalid design document: {error}")))
}

pub(super) fn system_id(arguments: &Value) -> Result<AgenticSystemId, ToolError> {
    AgenticSystemId::new(text(arguments, "system_id")?).map_err(Into::into)
}

pub(super) fn execution_id(arguments: &Value) -> Result<AgenticSystemExecutionId, ToolError> {
    AgenticSystemExecutionId::new(text(arguments, "execution_id")?).map_err(Into::into)
}

/// An absent revision reads the head; a present one must be a real
/// revision rather than a placeholder.
pub(super) fn revision(arguments: &Value) -> Result<Option<AgenticSystemRevision>, ToolError> {
    let Some(raw) = arguments.get("revision") else {
        return Ok(None);
    };
    let raw = raw
        .as_u64()
        .ok_or_else(|| ToolError::invalid_request("field `revision` must be a whole number"))?;
    AgenticSystemRevision::new(raw)
        .map(Some)
        .map_err(Into::into)
}

pub(super) fn required_revision(arguments: &Value) -> Result<AgenticSystemRevision, ToolError> {
    revision(arguments)?.ok_or_else(|| ToolError::invalid_request("field `revision` is required"))
}

pub(super) fn optional_execution_id(
    arguments: &Value,
) -> Result<Option<AgenticSystemExecutionId>, ToolError> {
    match arguments.get("execution_id").and_then(Value::as_str) {
        None => Ok(None),
        Some(raw) => AgenticSystemExecutionId::new(raw)
            .map(Some)
            .map_err(Into::into),
    }
}

pub(super) fn list_query(arguments: &Value) -> Result<AgenticSystemQuery, ToolError> {
    let lifecycle = match arguments.get("lifecycle") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            serde_json::from_value::<AgenticSystemLifecycle>(value.clone()).map_err(|_| {
                ToolError::invalid_request("field `lifecycle` is not a known lifecycle")
            })?,
        ),
    };
    let limit = match arguments.get("limit") {
        None | Some(Value::Null) => AgenticSystemPageLimit::default(),
        Some(value) => {
            let raw = value.as_u64().and_then(|raw| u16::try_from(raw).ok());
            let raw =
                raw.ok_or_else(|| ToolError::invalid_request("field `limit` is out of range"))?;
            AgenticSystemPageLimit::new(raw)?
        }
    };
    let after = match arguments.get("after").and_then(Value::as_str) {
        None => None,
        Some(raw) => Some(AgenticSystemId::new(raw)?),
    };
    Ok(AgenticSystemQuery::new(lifecycle, limit, after))
}

pub(super) fn instantiate_input(
    arguments: &Value,
) -> Result<InstantiateAgenticSystemInput, ToolError> {
    Ok(InstantiateAgenticSystemInput::new(
        system_id(arguments)?,
        required_revision(arguments)?,
        execution_id(arguments)?,
        contexts(arguments)?,
        offers(arguments)?,
        destination(arguments)?,
        text(arguments, "actor_id")?,
        actor_kind(arguments)?,
    ))
}

pub(super) fn actor(arguments: &Value) -> Result<(AuditActorId, AuditActorKind), ToolError> {
    Ok((
        AuditActorId::new(text(arguments, "actor_id")?),
        actor_kind(arguments)?,
    ))
}

fn contexts(arguments: &Value) -> Result<BTreeMap<SystemCeremonyId, CeremonyContext>, ToolError> {
    let Some(inputs) = arguments.get("inputs").and_then(Value::as_object) else {
        return Ok(BTreeMap::new());
    };
    let mut contexts = BTreeMap::new();
    for (ceremony, context) in inputs {
        let entries = context
            .as_object()
            .ok_or_else(|| ToolError::invalid_request("each entry of `inputs` is an object"))?;
        contexts.insert(
            SystemCeremonyId::new(ceremony)?,
            CeremonyContext::new(Attributes::new(
                entries.clone().into_iter().collect::<BTreeMap<_, _>>(),
            )?),
        );
    }
    Ok(contexts)
}

fn offers(arguments: &Value) -> Result<BTreeMap<ParticipantId, ParticipantOffer>, ToolError> {
    let Some(offered) = arguments.get("offers").and_then(Value::as_object) else {
        return Ok(BTreeMap::new());
    };
    let mut offers = BTreeMap::new();
    for (participant, offer) in offered {
        let specialty = offer
            .get("specialty")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::invalid_request("each offer needs a `specialty`"))?;
        let capabilities = offer
            .get("capabilities")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(Capability::new)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        offers.insert(
            ParticipantId::new(participant)?,
            ParticipantOffer::new(Specialty::new(specialty)?, capabilities),
        );
    }
    Ok(offers)
}

fn destination(arguments: &Value) -> Result<Option<HostDestination>, ToolError> {
    let Some(destination) = arguments.get("integrator_destination") else {
        return Ok(None);
    };
    if destination.is_null() {
        return Ok(None);
    }
    let host_kind = destination
        .get("host_kind")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::invalid_request("a destination needs a `host_kind`"))?;
    let address = destination
        .get("address")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::invalid_request("a destination needs an `address`"))?;
    // An unset mode is `none`: a destination that did not ask to be
    // woken is not woken, and guessing otherwise runs somebody's
    // command.
    let mode = match destination.get("activation_mode").and_then(Value::as_str) {
        None | Some("none") => HostActivationMode::None,
        Some("command") => HostActivationMode::Command,
        Some(_) => {
            return Err(ToolError::invalid_request(
                "field `activation_mode` is `none` or `command`",
            ))
        }
    };
    Ok(Some(HostDestination::new(
        HostKind::new(host_kind)?,
        HostAddress::new(address)?,
        mode,
    )))
}

fn actor_kind(arguments: &Value) -> Result<AuditActorKind, ToolError> {
    let raw = arguments
        .get("actor_kind")
        .cloned()
        .ok_or_else(|| ToolError::invalid_request("field `actor_kind` is required"))?;
    serde_json::from_value(raw)
        .map_err(|_| ToolError::invalid_request("field `actor_kind` is not a known kind"))
}

fn text<'value>(arguments: &'value Value, field: &str) -> Result<&'value str, ToolError> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::invalid_request(format!("field `{field}` is required")))
}
