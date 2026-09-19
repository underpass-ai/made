use made_core::value_objects::{
    AuthorizationDecisionId, AuthorizationDecisionPageLimit, AuthorizationGrantId,
    AuthorizationRevocationReason,
};
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};

use super::{embedded_authorization_presenter as view, embedded_authorization_request as request};
use crate::protocol::ToolError;

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_get_authorization_policy"
            | "made_issue_authorization_grant"
            | "made_revoke_authorization_grant"
            | "made_approve_authorization_operation"
            | "made_list_authorization_decisions"
    )
}

pub(super) fn present_approval(
    decision: &made_core::value_objects::AuthorizationDecision,
) -> Result<Value, ToolError> {
    Ok(json!({"decision":view::decision(decision)?}))
}

pub(super) async fn dispatch(
    made: &EmbeddedMade,
    name: &str,
    args: &Value,
) -> Result<Value, ToolError> {
    match name {
        "made_get_authorization_policy" => {
            Ok(json!({"policy":view::policy(&made.authorization_policy().await?.policy)?}))
        }
        "made_issue_authorization_grant" => Ok(view::mutation(
            made.issue_authorization_grant(request::grant(args)?)
                .await?,
        )),
        "made_revoke_authorization_grant" => Ok(view::mutation(
            made.revoke_authorization_grant(
                &AuthorizationGrantId::new(request::string(args, "grant_id")?)?,
                AuthorizationRevocationReason::new(request::string(args, "reason")?)?,
            )
            .await?,
        )),
        "made_approve_authorization_operation" => Err(ToolError::refused(
            "operation approval requires the configured embedded authorization gate",
        )),
        "made_list_authorization_decisions" => {
            let after = request::optional_string(args, "after_decision_id")
                .map(AuthorizationDecisionId::new)
                .transpose()?;
            let limit = args
                .get("limit")
                .map(|value| {
                    value
                        .as_u64()
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or_else(|| {
                            ToolError::invalid_request("limit must be between 1 and 500")
                        })
                })
                .transpose()?
                .unwrap_or(100);
            let limit = AuthorizationDecisionPageLimit::new(limit)?;
            let page = made.authorization_decisions(after.as_ref(), limit).await?;
            let next = (page.decisions().len() == limit.value())
                .then(|| page.decisions().last())
                .flatten()
                .map(made_core::value_objects::AuthorizationDecision::id);
            Ok(
                json!({"decisions":page.decisions().iter().map(view::decision).collect::<Result<Vec<_>,_>>()?,"next_after_decision_id":next}),
            )
        }
        _ => Err(ToolError::invalid_request("unknown authorization tool")),
    }
}
