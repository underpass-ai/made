use made_mcp_proto::v1 as pb;
use serde_json::Value;

use super::super::json_to_proto as j2p;

pub(super) fn build_deliberate_request(args: &Value) -> Result<pb::DeliberateRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    let task_value = obj
        .get("task")
        .ok_or_else(|| "missing required `task` object".to_string())?;
    Ok(pb::DeliberateRequest {
        task: Some(j2p::task_from_json(task_value)?),
    })
}

pub(super) fn build_stream_deliberation_request(
    args: &Value,
) -> Result<pb::StreamDeliberationRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    let task_value = obj
        .get("task")
        .ok_or_else(|| "missing required `task` object".to_string())?;
    Ok(pb::StreamDeliberationRequest {
        task: Some(j2p::task_from_json(task_value)?),
    })
}

pub(super) fn build_get_deliberation_result_request(
    args: &Value,
) -> Result<pb::GetDeliberationResultRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::GetDeliberationResultRequest {
        task_id: j2p::require_str(obj, "task_id")?.to_string(),
    })
}

pub(super) fn build_orchestrate_request(args: &Value) -> Result<pb::OrchestrateRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    let task_value = obj
        .get("task")
        .ok_or_else(|| "missing required `task` object".to_string())?;
    Ok(pb::OrchestrateRequest {
        task: Some(j2p::task_from_json(task_value)?),
        execution_options: j2p::optional_pb_struct(obj, "execution_options")?,
    })
}

pub(super) fn build_create_council_request(
    args: &Value,
) -> Result<pb::CreateCouncilRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::CreateCouncilRequest {
        specialty: j2p::require_str(obj, "specialty")?.to_string(),
        num_agents: j2p::optional_u32(obj, "num_agents")?,
        agent_config: j2p::optional_pb_struct(obj, "agent_config")?,
    })
}

pub(super) fn build_list_councils_request(args: &Value) -> pb::ListCouncilsRequest {
    let include_agents = args
        .as_object()
        .is_some_and(|obj| j2p::optional_bool(obj, "include_agents"));
    pb::ListCouncilsRequest { include_agents }
}

pub(super) fn build_delete_council_request(
    args: &Value,
) -> Result<pb::DeleteCouncilRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::DeleteCouncilRequest {
        specialty: j2p::require_str(obj, "specialty")?.to_string(),
    })
}

pub(super) fn build_register_agent_request(
    args: &Value,
) -> Result<pb::RegisterAgentRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    let agent_value = obj
        .get("agent")
        .ok_or_else(|| "missing required `agent` object".to_string())?;
    Ok(pb::RegisterAgentRequest {
        specialty: j2p::require_str(obj, "specialty")?.to_string(),
        agent: Some(j2p::agent_summary_from_json(agent_value)?),
        agent_config: j2p::optional_pb_struct(obj, "agent_config")?,
    })
}

pub(super) fn build_unregister_agent_request(
    args: &Value,
) -> Result<pb::UnregisterAgentRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::UnregisterAgentRequest {
        agent_id: j2p::require_str(obj, "agent_id")?.to_string(),
    })
}

pub(super) fn build_process_trigger_event_request(
    args: &Value,
) -> Result<pb::ProcessTriggerEventRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    let event_value = obj
        .get("event")
        .ok_or_else(|| "missing required `event` object".to_string())?;
    Ok(pb::ProcessTriggerEventRequest {
        event: Some(j2p::trigger_event_from_json(event_value)?),
    })
}

pub(super) fn build_get_status_request(args: &Value) -> pb::GetStatusRequest {
    let include_stats = args
        .as_object()
        .is_some_and(|obj| j2p::optional_bool(obj, "include_stats"));
    pb::GetStatusRequest { include_stats }
}

pub(super) fn build_run_council_decision_request(
    args: &Value,
) -> Result<pb::RunCouncilDecisionRequest, String> {
    j2p::run_council_decision_request_from_json(args)
}

pub(super) fn build_run_ceremony_request(args: &Value) -> Result<pb::RunCeremonyRequest, String> {
    j2p::run_ceremony_request_from_json(args)
}

pub(super) fn build_register_contract_request(
    args: &Value,
) -> Result<pb::RegisterContractRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    let contract_value = obj
        .get("contract")
        .ok_or_else(|| "missing required `contract` object".to_string())?;
    Ok(pb::RegisterContractRequest {
        contract: Some(j2p::output_contract_from_json(contract_value)?),
    })
}

pub(super) fn build_delete_contract_request(
    args: &Value,
) -> Result<pb::DeleteContractRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::DeleteContractRequest {
        contract_id: j2p::require_str(obj, "contract_id")?.to_string(),
    })
}
