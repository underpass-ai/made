//! Turning wire messages into domain requests, and views into wire
//! messages.
//!
//! `Status` is tonic's error type and is large by nature; every
//! function here carries it, so the size lint is answered once for
//! the module rather than by boxing an error nobody ever stores.
//!
//! The design document itself is not mapped field by field: it is
//! handed to the one decoder the application owns, so the gRPC
//! backend and the in-process one cannot disagree about what a
//! document means.

#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;

use made_app::usecases::agentic_system::{
    AgenticSystemDesignDocument, AgenticSystemValidationView, AgenticSystemView,
    InstantiateAgenticSystemInput, ParticipantOffer,
};
use made_core::entities::AgenticSystemExecution;
use made_core::error::DomainError;
use made_core::ports::AgenticSystemQuery;
use made_core::value_objects::{
    AgenticSystemExecutionId, AgenticSystemId, AgenticSystemLifecycle, AgenticSystemPageLimit,
    AgenticSystemRevision, AuditActorId, AuditActorKind, CeremonyContext, HostDestination,
    ParticipantId, SystemCeremonyId,
};
use made_proto::v1 as pb;
use serde_json::Value;
use tonic::Status;

use crate::grpc::mappers::agentic_system::{
    context_from_struct, destination_from_proto, json_to_struct, struct_to_json,
};

use super::domain_error_to_status;

pub(super) fn design_document(
    request: pb::DesignAgenticSystemRequest,
) -> Result<AgenticSystemDesignDocument, Status> {
    let design = request
        .design
        .ok_or_else(|| Status::invalid_argument("a design document is required"))?;
    let json = struct_to_json(design).map_err(domain_error_to_status)?;
    serde_json::from_value(json)
        .map_err(|error| Status::invalid_argument(format!("invalid design document: {error}")))
}

pub(super) fn system_id(raw: &str) -> Result<AgenticSystemId, Status> {
    AgenticSystemId::new(raw).map_err(domain_error_to_status)
}

pub(super) fn execution_id(raw: &str) -> Result<AgenticSystemExecutionId, Status> {
    AgenticSystemExecutionId::new(raw).map_err(domain_error_to_status)
}

/// An empty identifier means "no run", not an invalid one.
pub(super) fn optional_execution_id(raw: &str) -> Result<Option<AgenticSystemExecutionId>, Status> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    execution_id(raw).map(Some)
}

/// Zero means the head, because proto3 cannot tell an unset number
/// from a zero one and a revision is never zero.
pub(super) fn revision(raw: u64) -> Result<Option<AgenticSystemRevision>, Status> {
    if raw == 0 {
        return Ok(None);
    }
    AgenticSystemRevision::new(raw)
        .map(Some)
        .map_err(domain_error_to_status)
}

pub(super) fn required_revision(raw: u64) -> Result<AgenticSystemRevision, Status> {
    AgenticSystemRevision::new(raw).map_err(domain_error_to_status)
}

pub(super) fn list_query(
    request: &pb::ListAgenticSystemsRequest,
) -> Result<AgenticSystemQuery, Status> {
    let lifecycle = match request.lifecycle.trim() {
        "" => None,
        raw => Some(
            serde_json::from_value::<AgenticSystemLifecycle>(Value::String(raw.to_owned()))
                .map_err(|_| Status::invalid_argument(format!("unknown lifecycle `{raw}`")))?,
        ),
    };
    let limit = match u16::try_from(request.limit).unwrap_or(u16::MAX) {
        0 => AgenticSystemPageLimit::default(),
        value => AgenticSystemPageLimit::new(value).map_err(domain_error_to_status)?,
    };
    let after = match request.after.trim() {
        "" => None,
        raw => Some(system_id(raw)?),
    };
    Ok(AgenticSystemQuery::new(lifecycle, limit, after))
}

pub(super) fn instantiate_input(
    request: pb::InstantiateAgenticSystemRequest,
) -> Result<InstantiateAgenticSystemInput, Status> {
    let inputs = request
        .inputs
        .into_iter()
        .map(|(ceremony, context)| {
            Ok((
                SystemCeremonyId::new(&ceremony)?,
                context_from_struct(context)?,
            ))
        })
        .collect::<Result<BTreeMap<SystemCeremonyId, CeremonyContext>, DomainError>>()
        .map_err(domain_error_to_status)?;
    let offers = request
        .offers
        .into_iter()
        .map(|(participant, offer)| {
            Ok((
                ParticipantId::new(&participant)?,
                ParticipantOffer::new(
                    made_core::value_objects::Specialty::new(&offer.specialty)?,
                    offer
                        .capabilities
                        .iter()
                        .map(made_core::value_objects::Capability::new)
                        .collect::<Result<Vec<_>, _>>()?,
                ),
            ))
        })
        .collect::<Result<BTreeMap<ParticipantId, ParticipantOffer>, DomainError>>()
        .map_err(domain_error_to_status)?;
    let destination: Option<HostDestination> = request
        .integrator_destination
        .map(destination_from_proto)
        .transpose()
        .map_err(domain_error_to_status)?;
    Ok(InstantiateAgenticSystemInput::new(
        system_id(&request.system_id)?,
        required_revision(request.revision)?,
        execution_id(&request.execution_id)?,
        inputs,
        offers,
        destination,
        request.actor_id,
        actor_kind(&request.actor_kind)?,
    ))
}

pub(super) fn advance_input(
    request: pb::AdvanceAgenticSystemExecutionRequest,
) -> Result<(AgenticSystemExecutionId, AuditActorId, AuditActorKind), Status> {
    Ok((
        execution_id(&request.execution_id)?,
        AuditActorId::new(request.actor_id),
        actor_kind(&request.actor_kind)?,
    ))
}

pub(super) fn system_state(view: &AgenticSystemView) -> Result<pb::AgenticSystemState, Status> {
    let rendered = crate::json::AgenticSystemJson::of(view).map_err(domain_error_to_status)?;
    Ok(pb::AgenticSystemState {
        system_id: view.system().id().as_str().to_owned(),
        revision: view.revision().get(),
        lifecycle: view.system().lifecycle().as_str().to_owned(),
        digest: view.digest().to_hex(),
        yaml: rendered
            .get("yaml")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        json: Some(json_to_struct(&rendered)),
    })
}

#[must_use]
/// A catalogue entry: what a design is, without the document.
///
/// A listing that carried every design in full would be a listing
/// nobody could afford to call, so the `yaml` field stays empty here
/// and the payload is the summary — the same one the in-process
/// backend answers a listing with.
pub(super) fn summary_state(
    system: &made_core::entities::AgenticSystem,
) -> Result<pb::AgenticSystemState, Status> {
    let rendered =
        crate::json::AgenticSystemJson::summary(system).map_err(domain_error_to_status)?;
    Ok(pb::AgenticSystemState {
        system_id: system.id().as_str().to_owned(),
        revision: system.revision().get(),
        lifecycle: system.lifecycle().as_str().to_owned(),
        digest: system.digest().map_err(domain_error_to_status)?.to_hex(),
        yaml: String::new(),
        json: Some(json_to_struct(&rendered)),
    })
}

pub(super) fn validation_response(
    view: &AgenticSystemValidationView,
    rendered: &Value,
) -> pb::ValidateAgenticSystemResponse {
    pb::ValidateAgenticSystemResponse {
        system_id: view.system().id().as_str().to_owned(),
        revision: view.system().revision().get(),
        publishable: view.is_publishable(),
        error_count: u32::try_from(view.error_count()).unwrap_or(u32::MAX),
        warning_count: u32::try_from(view.warning_count()).unwrap_or(u32::MAX),
        findings: findings(rendered),
        resolved_pins: view
            .resolved_pins()
            .iter()
            .map(|(ceremony, pin)| pb::AgenticSystemResolvedPin {
                ceremony: ceremony.as_str().to_owned(),
                name: pin.name().as_str().to_owned(),
                version: pin.version().as_str().to_owned(),
                digest: pin.digest().to_hex(),
            })
            .collect(),
    }
}

#[must_use]
pub(super) fn execution_state(
    execution: &AgenticSystemExecution,
    rendered: &Value,
) -> pb::AgenticSystemExecutionState {
    pb::AgenticSystemExecutionState {
        execution_id: execution.id().as_str().to_owned(),
        system_id: execution.system().id().as_str().to_owned(),
        revision: execution.system().revision().get(),
        digest: execution.system().digest().to_hex(),
        state: execution.state().as_str().to_owned(),
        integrator_binding_id: execution
            .integrator_binding()
            .map(|binding| binding.as_str().to_owned())
            .unwrap_or_default(),
        ceremonies: ceremonies(rendered),
        json: Some(json_to_struct(rendered)),
    }
}

fn findings(rendered: &Value) -> Vec<pb::CeremonyDraftFinding> {
    rendered
        .get("findings")
        .and_then(Value::as_array)
        .map(|findings| {
            findings
                .iter()
                .map(|finding| pb::CeremonyDraftFinding {
                    severity: text(finding, "severity"),
                    locus: finding.get("locus").map(json_to_struct),
                    message: text(finding, "message"),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn ceremonies(rendered: &Value) -> Vec<pb::AgenticSystemCeremonyState> {
    rendered
        .get("ceremonies")
        .and_then(Value::as_array)
        .map(|ceremonies| ceremonies.iter().map(ceremony_state).collect())
        .unwrap_or_default()
}

fn ceremony_state(rendered: &Value) -> pb::AgenticSystemCeremonyState {
    let pin = rendered.get("pin").cloned().unwrap_or(Value::Null);
    let observed = rendered.get("observed").cloned().unwrap_or(Value::Null);
    pb::AgenticSystemCeremonyState {
        ceremony: text(rendered, "ceremony"),
        pin_name: text(&pin, "name"),
        pin_version: text(&pin, "version"),
        pin_digest: text(&pin, "digest"),
        planned: text(rendered, "planned"),
        round: rendered
            .get("round")
            .and_then(Value::as_u64)
            .and_then(|round| u32::try_from(round).ok())
            .unwrap_or_default(),
        instance_id: text(rendered, "instance_id"),
        observed_phase: text(&observed, "phase"),
        observed_state: text(&observed, "state"),
        skipped_because: text(rendered, "skipped_because"),
    }
}

/// A missing or null value is the empty string on the wire, which is
/// what proto3 means by "not set".
fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn actor_kind(raw: &str) -> Result<AuditActorKind, Status> {
    serde_json::from_value(Value::String(raw.to_owned()))
        .map_err(|_| Status::invalid_argument(format!("unknown actor kind `{raw}`")))
}
