use made_client::v1::{
    AuthorizationDecisionRecord, AuthorizationGrantRecord, AuthorizationPolicyRecord,
    AuthorizationPrincipal, AuthorizationScope, ListAuthorizationDecisionsResponse,
};
use serde_json::{json, Value};

use crate::OutputFormat;

pub fn policy(policy: &AuthorizationPolicyRecord, format: OutputFormat) -> String {
    super::render::value(
        &json!({
            "policy_id": policy.policy_id,
            "version": policy.version,
            "owner": policy.owner.as_ref().map(principal),
            "grants": policy.grants.iter().map(grant).collect::<Vec<_>>(),
            "revocations": policy.revoked_grant_ids,
            "separation_rules": policy.separation_rules.iter().map(|rule| json!({
                "approval_action": rule.approval_action,
                "execution_action": rule.execution_action,
            })).collect::<Vec<_>>(),
        }),
        format,
    )
}

pub fn decisions(page: &ListAuthorizationDecisionsResponse, format: OutputFormat) -> String {
    super::render::value(
        &json!({
            "decisions": page.decisions.iter().map(decision).collect::<Vec<_>>(),
            "next_after_decision_id": page.next_after_decision_id,
        }),
        format,
    )
}

fn decision(value: &AuthorizationDecisionRecord) -> Value {
    json!({
        "decision_id": value.decision_id,
        "request_id": value.request_id,
        "principal": value.principal.as_ref().map(principal),
        "action": value.action,
        "scope": value.scope.as_ref().map(scope),
        "target_digest": value.target_digest,
        "approval_decision_id": value.approval_decision_id,
        "accepted_work_decision_id": value.accepted_work_decision_id,
        "policy_version": value.policy_version,
        "outcome": value.outcome,
        "grant_id": value.grant_id,
        "denial_reason": value.denial_reason,
        "decided_at": value.decided_at.as_ref().map(timestamp),
        "valid_until": value.valid_until.as_ref().map(timestamp),
    })
}

fn grant(value: &AuthorizationGrantRecord) -> Value {
    json!({
        "grant_id": value.grant_id,
        "grantee_id": value.grantee_id,
        "actions": value.actions,
        "scope": value.scope.as_ref().map(scope),
        "valid_from": value.valid_from.as_ref().map(timestamp),
        "valid_until": value.valid_until.as_ref().map(timestamp),
        "delegation_depth": value.delegation_depth,
        "issuer": value.issuer.as_ref().map(principal),
        "parent_grant_id": value.parent_grant_id,
    })
}

fn principal(value: &AuthorizationPrincipal) -> Value {
    json!({
        "principal_id": value.principal_id,
        "kind": value.kind,
        "authentication_method": value.authentication_method,
    })
}

fn scope(value: &AuthorizationScope) -> Value {
    json!({
        "kind": value.kind,
        "ceremony_id": value.ceremony_id,
        "root_id": value.root_id,
        "definition_name": value.definition_name,
        "definition_version": value.definition_version,
        "artifact_id": value.artifact_id,
        "council_id": value.council_id,
        "budget_account_id": value.budget_account_id,
    })
}

fn timestamp(value: &prost_types::Timestamp) -> Value {
    json!({"seconds": value.seconds, "nanos": value.nanos})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_render_the_accepted_work_authorization_antecedent() {
        let page = ListAuthorizationDecisionsResponse {
            decisions: vec![AuthorizationDecisionRecord {
                decision_id: "completion-decision".to_owned(),
                accepted_work_decision_id: Some("accepted-work-decision".to_owned()),
                ..AuthorizationDecisionRecord::default()
            }],
            ..ListAuthorizationDecisionsResponse::default()
        };

        let rendered: Value = serde_json::from_str(&decisions(&page, OutputFormat::Json)).unwrap();

        assert_eq!(
            rendered["decisions"][0]["accepted_work_decision_id"],
            json!("accepted-work-decision")
        );
    }
}
