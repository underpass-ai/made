use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

pub(crate) fn scope(value: &pb::AuthorizationScope) -> Value {
    match value.kind.as_str() {
        "global" => json!({"kind":"global"}),
        "ceremony" => json!({"kind":"ceremony","ceremony_id":value.ceremony_id}),
        "ceremony_tree" => json!({"kind":"ceremony_tree","root_id":value.root_id}),
        "resolved_ceremony" => {
            json!({"kind":"resolved_ceremony","ceremony_id":value.ceremony_id,"root_id":value.root_id})
        }
        "definition" => {
            json!({"kind":"definition","name":value.definition_name,"version":value.definition_version})
        }
        "artifact" => json!({"kind":"artifact","artifact_id":value.artifact_id}),
        "council" => json!({"kind":"council","council_id":value.council_id}),
        "budget" => json!({"kind":"budget","account_id":value.budget_account_id}),
        other => json!({"kind":other}),
    }
}

pub(super) fn evidence(value: &pb::CeremonyAuthorizationEvidence) -> Value {
    json!({
        "decision_id":value.decision_id,"request_id":value.request_id,
        "principal_id":value.principal_id,"action":value.action,
        "scope":value.scope.as_ref().map(scope),"target_digest":value.target_digest,
        "policy_version":value.policy_version,"admitted_at":value.admitted_at,
        "valid_until":value.valid_until,
    })
}
