use super::{
    bad_request, json, p2j, protocol_requests, Channel, MadeServiceClient, ToolError, Value,
};

pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_prepare_protocol_children"
            | "made_accept_child_completion"
            | "made_recover_protocol_children"
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
        "made_prepare_protocol_children" => {
            let request = protocol_requests::build_prepare_protocol_children_request(arguments)
                .map_err(bad_request)?;
            let response = client
                .prepare_protocol_children(request)
                .await?
                .into_inner();
            let instance = response
                .instance
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no protocol instance"))?;
            Ok(json!({
                "instance": instance,
                "child_group_id": response.child_group_id,
                "child_ids": response.child_ids,
            }))
        }
        "made_accept_child_completion" => {
            let request = protocol_requests::build_accept_child_completion_request(arguments)
                .map_err(bad_request)?;
            let response = client.accept_child_completion(request).await?.into_inner();
            let parent = response
                .parent
                .map(p2j::protocol_instance_state_to_json)
                .ok_or_else(|| ToolError::refused("made returned no parent protocol instance"))?;
            let completion = response
                .completion
                .map(|completion| p2j::child_completion_to_json(&completion))
                .ok_or_else(|| ToolError::refused("made returned no verified child completion"))?;
            Ok(json!({ "parent": parent, "completion": completion }))
        }
        "made_recover_protocol_children" => {
            let request = protocol_requests::build_recover_protocol_children_request(arguments)
                .map_err(bad_request)?;
            let response = client
                .recover_protocol_children(request)
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
            "unsupported child protocol tool `{name}`"
        ))),
    }
}
