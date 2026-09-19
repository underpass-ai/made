use made_mcp_proto::v1 as pb;
use serde_json::Value;

use super::super::json_to_proto as j2p;

pub(super) fn grant(args: &Value) -> Result<pb::IssueAuthorizationGrantRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    let actions = obj
        .get("actions")
        .and_then(Value::as_array)
        .ok_or_else(|| "actions must be an array".to_owned())?
        .iter()
        .map(|action| {
            action
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "each action must be a string".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let depth = obj
        .get("delegation_depth")
        .and_then(Value::as_u64)
        .ok_or_else(|| "delegation_depth must be an unsigned integer".to_owned())?;
    if depth > 8 {
        return Err("delegation_depth must be at most 8".to_owned());
    }
    Ok(pb::IssueAuthorizationGrantRequest {
        grant_id: j2p::require_str(obj, "grant_id")?.to_owned(),
        grantee_id: j2p::require_str(obj, "grantee_id")?.to_owned(),
        actions,
        scope: Some(scope(
            obj.get("scope")
                .ok_or_else(|| "scope is required".to_owned())?,
        )?),
        valid_from: Some(j2p::parse_rfc3339_to_timestamp(j2p::require_str(
            obj,
            "valid_from",
        )?)?),
        valid_until: j2p::optional_timestamp(obj, "valid_until")?,
        delegation_depth: u32::try_from(depth).map_err(|error| error.to_string())?,
        parent_grant_id: j2p::optional_str(obj, "parent_grant_id").map(str::to_owned),
    })
}

pub(super) fn scope(value: &Value) -> Result<pb::AuthorizationScope, String> {
    let obj = j2p::require_object(value, "scope")?;
    let kind = j2p::require_str(obj, "kind")?;
    let mut scope = pb::AuthorizationScope {
        kind: kind.to_owned(),
        ..Default::default()
    };
    match kind {
        "global" => {}
        "ceremony" => scope.ceremony_id = Some(j2p::require_str(obj, "ceremony_id")?.to_owned()),
        "ceremony_tree" => scope.root_id = Some(j2p::require_str(obj, "root_id")?.to_owned()),
        "definition" => {
            scope.definition_name = Some(j2p::require_str(obj, "name")?.to_owned());
            scope.definition_version = j2p::optional_str(obj, "version").map(str::to_owned);
        }
        "artifact" => scope.artifact_id = Some(j2p::require_str(obj, "artifact_id")?.to_owned()),
        "council" => scope.council_id = Some(j2p::require_str(obj, "council_id")?.to_owned()),
        "budget" => scope.budget_account_id = Some(j2p::require_str(obj, "account_id")?.to_owned()),
        _ => return Err(format!("unsupported grant scope `{kind}`")),
    }
    Ok(scope)
}

pub(super) fn revoke(args: &Value) -> Result<pb::RevokeAuthorizationGrantRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::RevokeAuthorizationGrantRequest {
        grant_id: j2p::require_str(obj, "grant_id")?.to_owned(),
        reason: j2p::require_str(obj, "reason")?.to_owned(),
    })
}

pub(super) fn decisions(args: &Value) -> Result<pb::ListAuthorizationDecisionsRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    let limit = match obj.get("limit") {
        None | Some(Value::Null) => 100,
        Some(value) => value
            .as_u64()
            .ok_or_else(|| "limit must be an unsigned integer".to_owned())?,
    };
    if !(1..=500).contains(&limit) {
        return Err("limit must be between 1 and 500".to_owned());
    }
    Ok(pb::ListAuthorizationDecisionsRequest {
        after_decision_id: j2p::optional_str(obj, "after_decision_id").map(str::to_owned),
        limit: u32::try_from(limit).map_err(|error| error.to_string())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::validate_tool_request;
    use serde_json::json;

    fn grant_args() -> Value {
        json!({"grant_id":"g-1","grantee_id":"worker-1","actions":["claim_ceremony_step"],
            "scope":{"kind":"ceremony_tree","root_id":"root-1"},
            "valid_from":"2026-09-19T00:00:00Z","valid_until":"2026-09-20T00:00:00Z",
            "delegation_depth":2,"parent_grant_id":"parent-1"})
    }

    #[test]
    fn authorization_grant_keeps_delegation_and_validity() {
        let value =
            validate_tool_request("made_issue_authorization_grant", &grant_args(), |_| true)
                .unwrap();
        let request = grant(&value).unwrap();
        assert_eq!(request.grantee_id, "worker-1");
        assert_eq!(request.scope.unwrap().root_id.as_deref(), Some("root-1"));
        assert_eq!(request.actions, vec!["claim_ceremony_step"]);
        assert_eq!(request.delegation_depth, 2);
        assert_eq!(request.parent_grant_id.as_deref(), Some("parent-1"));
        assert_eq!(
            request.valid_until.unwrap().seconds - request.valid_from.unwrap().seconds,
            86400
        );
    }

    #[test]
    fn authorization_grant_rejects_forged_identity_and_request_only_scope() {
        for field in ["issuer", "owner", "policy_id", "authentication_method"] {
            let mut args = grant_args();
            args[field] = json!("forged");
            assert!(
                validate_tool_request("made_issue_authorization_grant", &args, |_| true).is_err(),
                "{field}"
            );
        }
        for scope in [
            json!({"kind":"resolved_ceremony","ceremony_id":"child","root_id":"root"}),
            json!({"kind":"ceremony","ceremony_id":"child","root_id":"root"}),
            json!({"kind":"global","artifact_id":"victim"}),
            json!({"kind":"definition","name":"research","version":1}),
        ] {
            let mut args = grant_args();
            args["scope"] = scope;
            assert!(
                validate_tool_request("made_issue_authorization_grant", &args, |_| true).is_err()
            );
        }
    }

    #[test]
    fn authorization_scopes_preserve_public_resource_names() {
        let definition =
            scope(&json!({"kind":"definition","name":"research","version":"v1"})).unwrap();
        assert_eq!(definition.definition_name.as_deref(), Some("research"));
        assert_eq!(definition.definition_version.as_deref(), Some("v1"));
        assert_eq!(
            scope(&json!({"kind":"budget","account_id":"b-1"}))
                .unwrap()
                .budget_account_id
                .as_deref(),
            Some("b-1")
        );
        assert!(
            scope(&json!({"kind":"resolved_ceremony","ceremony_id":"child","root_id":"root"}))
                .is_err()
        );
    }

    #[test]
    fn authorization_limits_and_timestamps_refuse_bad_inputs() {
        for limit in [json!(0), json!(501), json!(-1), json!("10")] {
            assert!(decisions(&json!({"limit":limit})).is_err());
        }
        assert_eq!(decisions(&json!({})).unwrap().limit, 100);
        let mut args = grant_args();
        args["valid_from"] = json!("not-a-date");
        assert!(grant(&args).is_err());
        args = grant_args();
        args["delegation_depth"] = json!(9);
        assert!(grant(&args).is_err());
        args = grant_args();
        args["actions"] = json!(["claim_ceremony_step", "claim_ceremony_step"]);
        assert!(validate_tool_request("made_issue_authorization_grant", &args, |_| true).is_err());
    }
}
