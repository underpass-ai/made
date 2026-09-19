use made_app::services::AuthorizationOperationScope;
use made_core::value_objects::{
    ArtifactId, AuthorizationAction, AuthorizationGrant, AuthorizationGrantId,
    AuthorizationGrantIssuer, AuthorizationScope, BudgetAccountId, CeremonyId, CeremonyName,
    CeremonyVersion, CouncilId, DelegationDepth, PrincipalId,
};
use serde_json::Value;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::protocol::ToolError;

pub(super) fn grant(args: &Value) -> Result<AuthorizationGrant, ToolError> {
    let operation = AuthorizationOperationScope::current()
        .ok_or_else(|| ToolError::refused("authenticated authorization context is required"))?;
    let issuer = match optional_string(args, "parent_grant_id") {
        Some(parent) => AuthorizationGrantIssuer::delegated(
            operation.principal().clone(),
            AuthorizationGrantId::new(parent)?,
        ),
        None => AuthorizationGrantIssuer::direct(operation.principal().clone()),
    };
    let actions: Vec<AuthorizationAction> =
        serde_json::from_value(args.get("actions").cloned().unwrap_or(Value::Null))
            .map_err(|error| ToolError::invalid_request(error.to_string()))?;
    let depth = args
        .get("delegation_depth")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .ok_or_else(|| ToolError::invalid_request("delegation_depth must be between 0 and 8"))?;
    Ok(AuthorizationGrant::new(
        AuthorizationGrantId::new(string(args, "grant_id")?)?,
        PrincipalId::new(string(args, "grantee_id")?)?,
        actions,
        scope(args.get("scope").unwrap_or(&Value::Null))?,
        (
            timestamp(string(args, "valid_from")?)?,
            optional_string(args, "valid_until")
                .map(timestamp)
                .transpose()?,
        ),
        DelegationDepth::new(depth)?,
        issuer,
    )?)
}

fn scope(value: &Value) -> Result<AuthorizationScope, ToolError> {
    Ok(match string(value, "kind")? {
        "global" => AuthorizationScope::Global,
        "ceremony" => AuthorizationScope::Ceremony {
            ceremony_id: CeremonyId::new(string(value, "ceremony_id")?)?,
        },
        "ceremony_tree" => AuthorizationScope::CeremonyTree {
            root_id: CeremonyId::new(string(value, "root_id")?)?,
        },
        "definition" => AuthorizationScope::Definition {
            name: CeremonyName::new(string(value, "name")?)?,
            version: optional_string(value, "version")
                .map(CeremonyVersion::new)
                .transpose()?,
        },
        "artifact" => AuthorizationScope::Artifact {
            artifact_id: ArtifactId::new(string(value, "artifact_id")?)?,
        },
        "council" => AuthorizationScope::Council {
            council_id: CouncilId::new(string(value, "council_id")?)?,
        },
        "budget" => AuthorizationScope::Budget {
            account_id: BudgetAccountId::new(string(value, "account_id")?)?,
        },
        other => {
            return Err(ToolError::invalid_request(format!(
                "unsupported grant scope `{other}`"
            )))
        }
    })
}

pub(super) fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str, ToolError> {
    optional_string(value, field)
        .ok_or_else(|| ToolError::invalid_request(format!("{field} must be a string")))
}

pub(super) fn optional_string<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn timestamp(value: &str) -> Result<OffsetDateTime, ToolError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|error| ToolError::invalid_request(error.to_string()))
}
