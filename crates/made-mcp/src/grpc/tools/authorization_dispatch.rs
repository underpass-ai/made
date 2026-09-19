use super::{
    authorization_presenter as view, authorization_requests as request, bad_request, json, pb,
    Channel, MadeServiceClient, ToolError, Value,
};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_get_authorization_policy"
            | "made_issue_authorization_grant"
            | "made_revoke_authorization_grant"
            | "made_list_authorization_decisions"
    )
}

pub(super) async fn dispatch<I>(
    client: &mut MadeServiceClient<tonic::service::interceptor::InterceptedService<Channel, I>>,
    name: &str,
    arguments: &Value,
) -> Result<Value, ToolError>
where
    I: tonic::service::Interceptor + Clone + Send + Sync + 'static,
{
    match name {
        "made_get_authorization_policy" => {
            let response = client
                .get_authorization_policy(pb::GetAuthorizationPolicyRequest {})
                .await?
                .into_inner();
            let policy = response
                .policy
                .ok_or_else(|| ToolError::refused("made returned no authorization policy"))?;
            Ok(json!({"policy":view::policy(policy)}))
        }
        "made_issue_authorization_grant" => {
            let response = client
                .issue_authorization_grant(request::grant(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            Ok(json!({"version":response.version,"existing":response.existing}))
        }
        "made_revoke_authorization_grant" => {
            let response = client
                .revoke_authorization_grant(request::revoke(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            Ok(json!({"version":response.version,"existing":response.existing}))
        }
        "made_list_authorization_decisions" => {
            let response = client
                .list_authorization_decisions(request::decisions(arguments).map_err(bad_request)?)
                .await?
                .into_inner();
            Ok(
                json!({"decisions":response.decisions.iter().map(view::decision).collect::<Vec<_>>(),
                "next_after_decision_id":response.next_after_decision_id}),
            )
        }
        _ => Err(ToolError::invalid_request("unknown authorization tool")),
    }
}
