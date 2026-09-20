//! Tool arguments to agentic-system RPC requests.
//!
//! The design document is carried through as structured data rather
//! than picked apart here. This backend is a transport: deciding what
//! a document means belongs to the one decoder that owns it, on the
//! other side.

use super::super::json_to_proto as j2p;
use made_mcp_proto::v1 as pb;
use serde_json::Value;

pub(super) fn design(arguments: &Value) -> Result<pb::DesignAgenticSystemRequest, String> {
    let object = j2p::require_object(arguments, "arguments")?;
    let design = object.get("design").ok_or("missing design")?;
    Ok(pb::DesignAgenticSystemRequest {
        design: Some(j2p::json_object_to_pb_struct(
            design.as_object().ok_or("design must be an object")?,
        )),
    })
}

pub(super) fn get(arguments: &Value) -> Result<pb::GetAgenticSystemRequest, String> {
    let object = j2p::require_object(arguments, "arguments")?;
    Ok(pb::GetAgenticSystemRequest {
        system_id: j2p::require_str(object, "system_id")?.into(),
        revision: revision(arguments),
    })
}

pub(super) fn list(arguments: &Value) -> Result<pb::ListAgenticSystemsRequest, String> {
    let object = j2p::require_object(arguments, "arguments")?;
    let limit = object.get("limit").and_then(Value::as_u64).unwrap_or(0);
    Ok(pb::ListAgenticSystemsRequest {
        lifecycle: j2p::optional_str(object, "lifecycle")
            .unwrap_or_default()
            .into(),
        limit: u32::try_from(limit).map_err(|_| "invalid limit")?,
        after: j2p::optional_str(object, "after")
            .unwrap_or_default()
            .into(),
    })
}

pub(super) fn validate(arguments: &Value) -> Result<pb::ValidateAgenticSystemRequest, String> {
    let object = j2p::require_object(arguments, "arguments")?;
    Ok(pb::ValidateAgenticSystemRequest {
        system_id: j2p::require_str(object, "system_id")?.into(),
        revision: revision(arguments),
    })
}

pub(super) fn publish(arguments: &Value) -> Result<pb::PublishAgenticSystemRequest, String> {
    let object = j2p::require_object(arguments, "arguments")?;
    Ok(pb::PublishAgenticSystemRequest {
        system_id: j2p::require_str(object, "system_id")?.into(),
        revision: revision(arguments),
    })
}

pub(super) fn instantiate(
    arguments: &Value,
) -> Result<pb::InstantiateAgenticSystemRequest, String> {
    let object = j2p::require_object(arguments, "arguments")?;
    let inputs = object
        .get("inputs")
        .and_then(Value::as_object)
        .map(|inputs| {
            inputs
                .iter()
                .filter_map(|(ceremony, context)| {
                    context
                        .as_object()
                        .map(|context| (ceremony.clone(), j2p::json_object_to_pb_struct(context)))
                })
                .collect()
        })
        .unwrap_or_default();
    let offers = object
        .get("offers")
        .and_then(Value::as_object)
        .map(|offers| {
            offers
                .iter()
                .map(|(participant, offer)| (participant.clone(), participant_offer(offer)))
                .collect()
        })
        .unwrap_or_default();
    Ok(pb::InstantiateAgenticSystemRequest {
        system_id: j2p::require_str(object, "system_id")?.into(),
        revision: revision(arguments),
        execution_id: j2p::require_str(object, "execution_id")?.into(),
        inputs,
        offers,
        integrator_destination: object.get("integrator_destination").map(destination),
        actor_id: j2p::require_str(object, "actor_id")?.into(),
        actor_kind: j2p::require_str(object, "actor_kind")?.into(),
    })
}

pub(super) fn advance(
    arguments: &Value,
) -> Result<pb::AdvanceAgenticSystemExecutionRequest, String> {
    let object = j2p::require_object(arguments, "arguments")?;
    Ok(pb::AdvanceAgenticSystemExecutionRequest {
        execution_id: j2p::require_str(object, "execution_id")?.into(),
        actor_id: j2p::require_str(object, "actor_id")?.into(),
        actor_kind: j2p::require_str(object, "actor_kind")?.into(),
    })
}

pub(super) fn get_execution(
    arguments: &Value,
) -> Result<pb::GetAgenticSystemExecutionRequest, String> {
    let object = j2p::require_object(arguments, "arguments")?;
    Ok(pb::GetAgenticSystemExecutionRequest {
        execution_id: j2p::require_str(object, "execution_id")?.into(),
    })
}

pub(super) fn diagram(arguments: &Value) -> Result<pb::RenderAgenticSystemDiagramRequest, String> {
    let object = j2p::require_object(arguments, "arguments")?;
    Ok(pb::RenderAgenticSystemDiagramRequest {
        system_id: j2p::require_str(object, "system_id")?.into(),
        revision: revision(arguments),
        execution_id: j2p::optional_str(object, "execution_id")
            .unwrap_or_default()
            .into(),
    })
}

/// Zero is "not set" on the wire, and a revision is never zero, so an
/// omitted revision and an explicit head read the same.
fn revision(arguments: &Value) -> u64 {
    arguments
        .get("revision")
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn participant_offer(offer: &Value) -> pb::AgenticSystemParticipantOffer {
    pb::AgenticSystemParticipantOffer {
        specialty: offer
            .get("specialty")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        capabilities: offer
            .get("capabilities")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn destination(destination: &Value) -> pb::AgenticSystemIntegratorDestination {
    pb::AgenticSystemIntegratorDestination {
        host_kind: destination
            .get("host_kind")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        address: destination
            .get("address")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        activation_mode: destination
            .get("activation_mode")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    }
}
