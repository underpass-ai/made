use super::{
    ApiError, Attributes, AuditActorKind, CeremonyId, CeremonyIntervention,
    CeremonyInterventionContent, CeremonyInterventionId, CeremonyInterventionKind,
    CeremonyInterventionTarget, DomainError, InterventionResponseView, InterventionView,
    OffsetDateTime, RaiseInterventionRequest, RequestCeremonyInterventionInput,
    RespondToCeremonyInterventionInput, RespondToInterventionRequest, RoleId,
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

pub(super) fn parse_intervention_kind(raw: &str) -> Result<CeremonyInterventionKind, DomainError> {
    match raw {
        "opinion" => Ok(CeremonyInterventionKind::Opinion),
        "investigation" => Ok(CeremonyInterventionKind::Investigation),
        "action" => Ok(CeremonyInterventionKind::Action),
        _ => Err(DomainError::InvalidCharacters {
            field: "intervention_kind",
        }),
    }
}

pub(super) fn parse_target(
    role_ids: Vec<String>,
) -> Result<CeremonyInterventionTarget, DomainError> {
    if role_ids.is_empty() {
        return Ok(CeremonyInterventionTarget::Table);
    }
    let roles = role_ids
        .into_iter()
        .map(RoleId::new)
        .collect::<Result<Vec<_>, _>>()?;
    CeremonyInterventionTarget::roles(roles)
}

pub(super) fn raise_input(
    request: RaiseInterventionRequest,
) -> Result<RequestCeremonyInterventionInput, DomainError> {
    Ok(RequestCeremonyInterventionInput::new(
        CeremonyId::new(request.ceremony_id)?,
        CeremonyInterventionId::new(request.intervention_id)?,
        RoleId::new(request.role_id)?,
        parse_actor_kind(&request.role_kind)?,
        parse_intervention_kind(&request.kind)?,
        parse_target(request.target_role_ids)?,
        CeremonyInterventionContent::new(request.request, Attributes::empty())?,
    ))
}

pub(super) fn respond_input(
    request: RespondToInterventionRequest,
) -> Result<RespondToCeremonyInterventionInput, DomainError> {
    Ok(RespondToCeremonyInterventionInput::new(
        CeremonyId::new(request.ceremony_id)?,
        CeremonyInterventionId::new(request.intervention_id)?,
        RoleId::new(request.role_id)?,
        parse_actor_kind(&request.role_kind)?,
        CeremonyInterventionContent::new(request.content, Attributes::empty())?,
    ))
}

pub(super) fn intervention_view(intervention: &CeremonyIntervention) -> InterventionView {
    InterventionView {
        intervention_id: intervention.id().as_str().to_owned(),
        kind: intervention.kind().as_label().to_owned(),
        requested_by: intervention.requested_by().as_str().to_owned(),
        target_role_ids: match intervention.target() {
            CeremonyInterventionTarget::Table => Vec::new(),
            CeremonyInterventionTarget::Roles(roles) => roles
                .as_set()
                .iter()
                .map(|role| role.as_str().to_owned())
                .collect(),
            // The versioned API names seats, not processes. An item put
            // to one live agent is shown as put to the seat that agent
            // holds, which is true and is all this shape can say.
            CeremonyInterventionTarget::AgentExecution(recipient) => {
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
