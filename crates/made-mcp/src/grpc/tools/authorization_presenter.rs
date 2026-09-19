use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

use super::p2j::{authorization_scope_to_json as scope, timestamp_to_rfc3339};

pub(super) fn policy(policy: pb::AuthorizationPolicyRecord) -> Value {
    json!({
        "policy_id":policy.policy_id,"version":policy.version,
        "owner":policy.owner.as_ref().map(principal),
        "grants":policy.grants.iter().map(grant).collect::<Vec<_>>(),
        "revocations":policy.revoked_grant_ids,
        "separation_rules":policy.separation_rules.into_iter().map(|rule|
            json!({"approval_action":rule.approval_action,"execution_action":rule.execution_action}))
            .collect::<Vec<_>>()
    })
}

fn principal(value: &pb::AuthorizationPrincipal) -> Value {
    json!({"principal_id":value.principal_id,"kind":value.kind,"authentication_method":value.authentication_method})
}

fn grant(value: &pb::AuthorizationGrantRecord) -> Value {
    json!({
        "grant_id":value.grant_id,"grantee_id":value.grantee_id,"actions":value.actions,
        "scope":value.scope.as_ref().map(scope),"valid_from":timestamp_to_rfc3339(value.valid_from.as_ref()),
        "valid_until":timestamp_to_rfc3339(value.valid_until.as_ref()),
        "delegation_depth":value.delegation_depth,"issuer":value.issuer.as_ref().map(principal),
        "parent_grant_id":value.parent_grant_id
    })
}

pub(super) fn decision(value: &pb::AuthorizationDecisionRecord) -> Value {
    json!({
        "decision_id":value.decision_id,"request_id":value.request_id,
        "principal":value.principal.as_ref().map(principal),"action":value.action,
        "scope":value.scope.as_ref().map(scope),"target_digest":value.target_digest,
        "approval_decision_id":value.approval_decision_id,
        "approved_action":value.approved_action,
        "accepted_work_decision_id":value.accepted_work_decision_id,"policy_version":value.policy_version,
        "outcome":value.outcome,"grant_id":value.grant_id,"denial_reason":value.denial_reason,
        "decided_at":timestamp_to_rfc3339(value.decided_at.as_ref()),
        "valid_until":timestamp_to_rfc3339(value.valid_until.as_ref())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor() -> pb::AuthorizationPrincipal {
        pb::AuthorizationPrincipal {
            principal_id: "service-reader".to_owned(),
            kind: "service_account".to_owned(),
            authentication_method: "mutual_tls".to_owned(),
        }
    }

    #[test]
    fn authorization_decision_keeps_denial_and_resolved_scope_evidence() {
        let response = decision(&pb::AuthorizationDecisionRecord {
            decision_id: "decision-1".to_owned(),
            request_id: "request-1".to_owned(),
            principal: Some(actor()),
            action: "get_ceremony_instance".to_owned(),
            scope: Some(pb::AuthorizationScope {
                kind: "resolved_ceremony".to_owned(),
                ceremony_id: Some("child".to_owned()),
                root_id: Some("root".to_owned()),
                ..Default::default()
            }),
            target_digest: "a".repeat(64),
            approval_decision_id: Some("approval-1".to_owned()),
            policy_version: 3,
            outcome: "denied".to_owned(),
            denial_reason: Some("no_matching_grant".to_owned()),
            decided_at: Some(prost_types::Timestamp {
                seconds: 0,
                nanos: 0,
            }),
            valid_until: Some(prost_types::Timestamp {
                seconds: 30,
                nanos: 0,
            }),
            ..Default::default()
        });
        assert_eq!(response["principal"]["principal_id"], "service-reader");
        assert_eq!(response["principal"]["authentication_method"], "mutual_tls");
        assert_eq!(
            response["scope"],
            json!({"kind":"resolved_ceremony","ceremony_id":"child","root_id":"root"})
        );
        assert_eq!(response["target_digest"], "a".repeat(64));
        assert_eq!(response["approval_decision_id"], "approval-1");
        assert_eq!(response["denial_reason"], "no_matching_grant");
        assert_eq!(response["decided_at"], "1970-01-01T00:00:00Z");
        assert_eq!(response["valid_until"], "1970-01-01T00:00:30Z");
    }

    #[test]
    fn authorization_policy_preserves_revocations_and_separation_rules() {
        let response = policy(pb::AuthorizationPolicyRecord {
            policy_id: "p".to_owned(),
            version: 4,
            owner: Some(actor()),
            revoked_grant_ids: vec!["revoked-1".to_owned()],
            separation_rules: vec![pb::AuthorizationSeparationRule {
                approval_action: "approve_ceremony_guard".to_owned(),
                execution_action: "claim_ceremony_step".to_owned(),
            }],
            ..Default::default()
        });
        assert_eq!(response["revocations"], json!(["revoked-1"]));
        assert_eq!(
            response["separation_rules"][0]["execution_action"],
            "claim_ceremony_step"
        );
        assert_eq!(response["version"], 4);
    }
}
