use made_core::entities::AuthorizationPolicy;
use made_core::value_objects::{
    ArtifactId, AuthenticatedPrincipal, AuthorizationAction, AuthorizationDecision,
    AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationScope,
    BudgetAccountId, CeremonyId, CeremonyName, CeremonyVersion, CouncilId, DelegationDepth,
    PrincipalId, SeparationRule,
};
use made_core::DomainError;
use made_proto::v1 as pb;

use super::{offset_to_timestamp, timestamp_to_offset};

pub fn authorization_action_from_proto(value: &str) -> Result<AuthorizationAction, DomainError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|_| {
        DomainError::InvalidDocument {
            reason: format!("unknown authorization action `{value}`"),
        }
    })
}

pub fn authorization_action_to_proto(value: AuthorizationAction) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{value:?}"))
}

pub fn authorization_scope_from_proto(
    value: pb::AuthorizationScope,
) -> Result<AuthorizationScope, DomainError> {
    match value.kind.as_str() {
        "global" => Ok(AuthorizationScope::Global),
        "ceremony" => Ok(AuthorizationScope::Ceremony {
            ceremony_id: CeremonyId::new(required(value.ceremony_id, "ceremony_id")?)?,
        }),
        "ceremony_tree" => Ok(AuthorizationScope::CeremonyTree {
            root_id: CeremonyId::new(required(value.root_id, "root_id")?)?,
        }),
        "definition" => Ok(AuthorizationScope::Definition {
            name: CeremonyName::new(required(value.definition_name, "definition_name")?)?,
            version: value
                .definition_version
                .map(CeremonyVersion::new)
                .transpose()?,
        }),
        "artifact" => Ok(AuthorizationScope::Artifact {
            artifact_id: ArtifactId::new(required(value.artifact_id, "artifact_id")?)?,
        }),
        "council" => Ok(AuthorizationScope::Council {
            council_id: CouncilId::new(required(value.council_id, "council_id")?)?,
        }),
        "budget" => Ok(AuthorizationScope::Budget {
            account_id: BudgetAccountId::new(required(
                value.budget_account_id,
                "budget_account_id",
            )?)?,
        }),
        other => Err(DomainError::InvalidDocument {
            reason: format!("unknown authorization scope `{other}`"),
        }),
    }
}

pub fn authorization_grant_from_proto(
    value: pb::IssueAuthorizationGrantRequest,
    issuer: &AuthenticatedPrincipal,
) -> Result<AuthorizationGrant, DomainError> {
    let issued_by = match value.parent_grant_id {
        Some(parent) => {
            AuthorizationGrantIssuer::delegated(issuer.clone(), AuthorizationGrantId::new(parent)?)
        }
        None => AuthorizationGrantIssuer::direct(issuer.clone()),
    };
    AuthorizationGrant::new(
        AuthorizationGrantId::new(value.grant_id)?,
        PrincipalId::new(value.grantee_id)?,
        value
            .actions
            .iter()
            .map(|action| authorization_action_from_proto(action))
            .collect::<Result<Vec<_>, _>>()?,
        authorization_scope_from_proto(value.scope.ok_or(DomainError::NotFound {
            what: "authorization_scope",
        })?)?,
        (
            timestamp_to_offset(value.valid_from.ok_or(DomainError::NotFound {
                what: "authorization_grant.valid_from",
            })?)?,
            value.valid_until.map(timestamp_to_offset).transpose()?,
        ),
        DelegationDepth::new(u8::try_from(value.delegation_depth).map_err(|_| {
            DomainError::OutOfRange {
                field: "delegation_depth",
                value: f64::from(value.delegation_depth),
                min: 0.0,
                max: f64::from(u8::MAX),
            }
        })?)?,
        issued_by,
    )
}

pub fn authorization_policy_to_proto(
    policy: &AuthorizationPolicy,
) -> pb::AuthorizationPolicyRecord {
    pb::AuthorizationPolicyRecord {
        policy_id: policy
            .id()
            .map_or_else(String::new, |id| id.as_str().to_owned()),
        version: policy.version().value(),
        owner: policy.owner().map(authorization_principal_to_proto),
        grants: policy.grants().map(authorization_grant_to_proto).collect(),
        revoked_grant_ids: policy
            .revoked_grant_ids()
            .map(|id| id.as_str().to_owned())
            .collect(),
        separation_rules: policy
            .separation_rules()
            .map(authorization_separation_rule_to_proto)
            .collect(),
    }
}

pub fn authorization_decision_to_proto(
    decision: &AuthorizationDecision,
) -> pb::AuthorizationDecisionRecord {
    let request = decision.request();
    pb::AuthorizationDecisionRecord {
        decision_id: decision.id().as_str().to_owned(),
        request_id: request.id().as_str().to_owned(),
        principal: Some(authorization_principal_to_proto(request.principal())),
        action: authorization_action_to_proto(request.action()),
        scope: Some(authorization_scope_to_proto(request.scope())),
        target_digest: request.target_digest().as_str().to_owned(),
        accepted_work_decision_id: request
            .accepted_work_decision_id()
            .map(|id| id.as_str().to_owned()),
        approval_decision_id: request
            .approval_decision_id()
            .map(|id| id.as_str().to_owned()),
        approved_action: request.approved_action().map(authorization_action_to_proto),
        policy_version: decision.policy_version().value(),
        outcome: enum_to_string(decision.kind()),
        grant_id: decision.grant_id().map(|id| id.as_str().to_owned()),
        denial_reason: decision.denial_reason().map(enum_to_string),
        decided_at: Some(offset_to_timestamp(decision.decided_at())),
        valid_until: Some(offset_to_timestamp(decision.valid_until())),
    }
}

fn authorization_grant_to_proto(grant: &AuthorizationGrant) -> pb::AuthorizationGrantRecord {
    pb::AuthorizationGrantRecord {
        grant_id: grant.id().as_str().to_owned(),
        grantee_id: grant.grantee().as_str().to_owned(),
        actions: grant
            .actions()
            .iter()
            .copied()
            .map(authorization_action_to_proto)
            .collect(),
        scope: Some(authorization_scope_to_proto(grant.scope())),
        valid_from: Some(offset_to_timestamp(grant.valid_from())),
        valid_until: grant.valid_until().map(offset_to_timestamp),
        delegation_depth: u32::from(grant.delegation_depth().value()),
        issuer: Some(authorization_principal_to_proto(grant.issued_by())),
        parent_grant_id: grant.parent_grant_id().map(|id| id.as_str().to_owned()),
    }
}

fn authorization_principal_to_proto(value: &AuthenticatedPrincipal) -> pb::AuthorizationPrincipal {
    pb::AuthorizationPrincipal {
        principal_id: value.id().as_str().to_owned(),
        kind: enum_to_string(value.kind()),
        authentication_method: enum_to_string(value.method()),
    }
}

pub(super) fn authorization_scope_to_proto(value: &AuthorizationScope) -> pb::AuthorizationScope {
    match value {
        AuthorizationScope::Global => pb::AuthorizationScope {
            kind: "global".to_owned(),
            ..pb::AuthorizationScope::default()
        },
        AuthorizationScope::Ceremony { ceremony_id } => pb::AuthorizationScope {
            kind: "ceremony".to_owned(),
            ceremony_id: Some(ceremony_id.as_str().to_owned()),
            ..pb::AuthorizationScope::default()
        },
        AuthorizationScope::CeremonyTree { root_id } => pb::AuthorizationScope {
            kind: "ceremony_tree".to_owned(),
            root_id: Some(root_id.as_str().to_owned()),
            ..pb::AuthorizationScope::default()
        },
        AuthorizationScope::ResolvedCeremony {
            ceremony_id,
            root_id,
        } => pb::AuthorizationScope {
            kind: "resolved_ceremony".to_owned(),
            ceremony_id: Some(ceremony_id.as_str().to_owned()),
            root_id: Some(root_id.as_str().to_owned()),
            ..pb::AuthorizationScope::default()
        },
        AuthorizationScope::Definition { name, version } => pb::AuthorizationScope {
            kind: "definition".to_owned(),
            definition_name: Some(name.as_str().to_owned()),
            definition_version: version.as_ref().map(|value| value.as_str().to_owned()),
            ..pb::AuthorizationScope::default()
        },
        AuthorizationScope::Artifact { artifact_id } => pb::AuthorizationScope {
            kind: "artifact".to_owned(),
            artifact_id: Some(artifact_id.as_str().to_owned()),
            ..pb::AuthorizationScope::default()
        },
        AuthorizationScope::Council { council_id } => pb::AuthorizationScope {
            kind: "council".to_owned(),
            council_id: Some(council_id.as_str().to_owned()),
            ..pb::AuthorizationScope::default()
        },
        AuthorizationScope::Budget { account_id } => pb::AuthorizationScope {
            kind: "budget".to_owned(),
            budget_account_id: Some(account_id.as_str().to_owned()),
            ..pb::AuthorizationScope::default()
        },
    }
}

fn authorization_separation_rule_to_proto(
    value: SeparationRule,
) -> pb::AuthorizationSeparationRule {
    pb::AuthorizationSeparationRule {
        approval_action: authorization_action_to_proto(value.approval_action()),
        execution_action: authorization_action_to_proto(value.execution_action()),
    }
}

fn enum_to_string<T: serde::Serialize + std::fmt::Debug>(value: T) -> String {
    serde_json::to_value(&value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{value:?}"))
}

fn required(value: Option<String>, field: &'static str) -> Result<String, DomainError> {
    value.ok_or(DomainError::NotFound { what: field })
}
