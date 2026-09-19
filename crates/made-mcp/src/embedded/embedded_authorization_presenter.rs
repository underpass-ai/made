use made_app::authorization::AuthorizationMutationOutcome;
use made_core::entities::AuthorizationPolicy;
use made_core::value_objects::{AuthenticatedPrincipal, AuthorizationDecision, AuthorizationGrant};
use serde_json::{json, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::protocol::ToolError;

pub(super) fn policy(value: &AuthorizationPolicy) -> Result<Value, ToolError> {
    Ok(json!({
        "policy_id":value.id(), "version":value.version(), "owner":value.owner().map(principal),
        "grants":value.grants().map(grant).collect::<Result<Vec<_>,_>>()?,
        "revocations":value.revoked_grant_ids().collect::<Vec<_>>(),
        "separation_rules":value.separation_rules().map(|rule| json!({
            "approval_action":rule.approval_action(),"execution_action":rule.execution_action()
        })).collect::<Vec<_>>()
    }))
}

fn principal(value: &AuthenticatedPrincipal) -> Value {
    json!({"principal_id":value.id(),"kind":value.kind(),"authentication_method":value.method()})
}

fn grant(value: &AuthorizationGrant) -> Result<Value, ToolError> {
    Ok(json!({
        "grant_id":value.id(), "grantee_id":value.grantee(), "actions":value.actions(),
        "scope":value.scope(), "valid_from":timestamp(value.valid_from())?,
        "valid_until":value.valid_until().map(timestamp).transpose()?,
        "delegation_depth":value.delegation_depth(), "issuer":principal(value.issued_by()),
        "parent_grant_id":value.parent_grant_id()
    }))
}

pub(super) fn decision(value: &AuthorizationDecision) -> Result<Value, ToolError> {
    let request = value.request();
    Ok(json!({
        "decision_id":value.id(),"request_id":request.id(),"principal":principal(request.principal()),
        "action":request.action(),"scope":request.scope(),"target_digest":request.target_digest(),
        "approval_decision_id":request.approval_decision_id(),
        "accepted_work_decision_id":request.accepted_work_decision_id(),"policy_version":value.policy_version(),
        "outcome":value.kind(),"grant_id":value.grant_id(),"denial_reason":value.denial_reason(),
        "decided_at":timestamp(value.decided_at())?,"valid_until":timestamp(value.valid_until())?
    }))
}

pub(super) fn mutation(value: AuthorizationMutationOutcome) -> Value {
    match value {
        AuthorizationMutationOutcome::Applied { version } => {
            json!({"version":version,"existing":false})
        }
        AuthorizationMutationOutcome::Existing { version } => {
            json!({"version":version,"existing":true})
        }
    }
}

fn timestamp(value: OffsetDateTime) -> Result<String, ToolError> {
    value
        .format(&Rfc3339)
        .map_err(|error| ToolError::refused(error.to_string()))
}
