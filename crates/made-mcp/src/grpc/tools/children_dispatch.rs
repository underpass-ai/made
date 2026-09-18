use super::{
    bad_request, ceremony_requests, json, p2j, Channel, MadeServiceClient, ToolError, Value,
};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_prepare_ceremony_children"
            | "made_accept_child_completion"
            | "made_recover_ceremony_children"
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
        "made_prepare_ceremony_children" => {
            let request = ceremony_requests::build_prepare_ceremony_children_request(arguments)
                .map_err(bad_request)?;
            let response = client
                .prepare_ceremony_children(request)
                .await?
                .into_inner();
            let instance = response
                .instance
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no ceremony instance"))?;
            Ok(json!({
                "instance": instance,
                "child_group_id": response.child_group_id,
                "child_ids": response.child_ids,
            }))
        }
        "made_accept_child_completion" => {
            let request = ceremony_requests::build_accept_child_completion_request(arguments)
                .map_err(bad_request)?;
            let response = client.accept_child_completion(request).await?.into_inner();
            let parent = response
                .parent
                .map(p2j::ceremony_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no parent ceremony instance"))?;
            let completion = response
                .completion
                .map(|completion| p2j::child_completion_to_json(&completion))
                .ok_or_else(|| ToolError::refused("made returned no verified child completion"))?;
            Ok(json!({ "parent": parent, "completion": completion }))
        }
        "made_recover_ceremony_children" => {
            let request = ceremony_requests::build_recover_ceremony_children_request(arguments)
                .map_err(bad_request)?;
            let response = client
                .recover_ceremony_children(request)
                .await?
                .into_inner();
            Ok(json!({
                "recovered_plans": response.recovered_plans,
                "accepted_completions": response.accepted_completions,
                "skipped": response.skipped,
                "failed": response.failed,
                "busy": response.busy,
            }))
        }
        _ => Err(ToolError::invalid_request(format!(
            "unsupported child ceremony tool `{name}`"
        ))),
    }
}
