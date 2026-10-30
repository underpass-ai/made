use super::{
    ApiError, Attributes, AuditActorKind, DomainError, InterventionResponseView, InterventionView,
    OffsetDateTime, ProtocolId, ProtocolIntervention, ProtocolInterventionContent,
    ProtocolInterventionId, ProtocolInterventionKind, ProtocolInterventionTarget,
    RaiseInterventionRequest, RequestProtocolInterventionInput, RespondToInterventionRequest,
    RespondToProtocolInterventionInput, RoleId,
};

pub(super) fn parse_actor_kind(raw: &str) -> Result<AuditActorKind, DomainError> {
    match raw {
        "human" => Ok(AuditActorKind::Human),
        "agent" => Ok(AuditActorKind::Agent),
        "service" => Ok(AuditActorKind::Service),
        "engine" => Ok(AuditActorKind::Engine),
        _ => Err(DomainError::InvalidCharacters {
            field: "actor_kind",
        }),
    }
}

pub(super) fn parse_intervention_kind(raw: &str) -> Result<ProtocolInterventionKind, DomainError> {
    match raw {
        "opinion" => Ok(ProtocolInterventionKind::Opinion),
        "investigation" => Ok(ProtocolInterventionKind::Investigation),
        "action" => Ok(ProtocolInterventionKind::Action),
        _ => Err(DomainError::InvalidCharacters {
            field: "intervention_kind",
        }),
    }
}

pub(super) fn parse_target(
    role_ids: Vec<String>,
) -> Result<ProtocolInterventionTarget, DomainError> {
    if role_ids.is_empty() {
        return Ok(ProtocolInterventionTarget::Table);
    }
    let roles = role_ids
        .into_iter()
        .map(RoleId::new)
        .collect::<Result<Vec<_>, _>>()?;
    ProtocolInterventionTarget::roles(roles)
}

pub(super) fn raise_input(
    request: RaiseInterventionRequest,
) -> Result<RequestProtocolInterventionInput, DomainError> {
    Ok(RequestProtocolInterventionInput::new(
        ProtocolId::new(request.protocol_id)?,
        ProtocolInterventionId::new(request.intervention_id)?,
        RoleId::new(request.role_id)?,
        parse_actor_kind(&request.role_kind)?,
        parse_intervention_kind(&request.kind)?,
        parse_target(request.target_role_ids)?,
        ProtocolInterventionContent::new(request.request, Attributes::empty())?,
    ))
}

pub(super) fn respond_input(
    request: RespondToInterventionRequest,
) -> Result<RespondToProtocolInterventionInput, DomainError> {
    Ok(RespondToProtocolInterventionInput::new(
        ProtocolId::new(request.protocol_id)?,
        ProtocolInterventionId::new(request.intervention_id)?,
        RoleId::new(request.role_id)?,
        parse_actor_kind(&request.role_kind)?,
        ProtocolInterventionContent::new(request.content, Attributes::empty())?,
    ))
}

pub(super) fn intervention_view(intervention: &ProtocolIntervention) -> InterventionView {
    InterventionView {
        intervention_id: intervention.id().as_str().to_owned(),
        kind: intervention.kind().as_label().to_owned(),
        requested_by: intervention.requested_by().as_str().to_owned(),
        target_role_ids: match intervention.target() {
            ProtocolInterventionTarget::Table => Vec::new(),
            ProtocolInterventionTarget::Roles(roles) => roles
                .as_set()
                .iter()
                .map(|role| role.as_str().to_owned())
                .collect(),
            // The versioned API names seats, not processes. An item put
            // to one live agent is shown as put to the seat that agent
            // holds, which is true and is all this shape can say.
            ProtocolInterventionTarget::AgentExecution(recipient) => {
                vec![recipient.role_id().as_str().to_owned()]
            }
        },
        request: intervention.request().message().to_owned(),
        open: intervention.status().is_open(),
        responses: intervention
            .responses()
            .iter()
            .map(|response| InterventionResponseView {
                role_id: response.role_id().as_str().to_owned(),
                content: response.content().message().to_owned(),
                responded_at_millis: millis(response.responded_at()),
            })
            .collect(),
        created_at_millis: millis(intervention.created_at()),
        closed_at_millis: intervention.closed_at().map(millis),
    }
}

pub(super) fn unavailable(error: &DomainError) -> ApiError {
    ApiError::Unavailable {
        reason: error.to_string(),
    }
}

pub(super) fn millis(at: OffsetDateTime) -> i64 {
    (at.unix_timestamp_nanos() / 1_000_000) as i64
}
