use super::{
    agent_summary_schema, attributes_schema, json, output_contract_schema,
    run_council_decision_schema, string_schema, task_schema, tool_def, trigger_event_schema, Value,
};

#[allow(clippy::too_many_lines)] // Council tool definitions form one auditable catalog slice.
pub(super) fn council_tool_catalog() -> Vec<Value> {
    vec![
        tool_def(
            "made_deliberate",
            "Run a deliberation on the council for the task's specialty. Returns ranked proposals once the council finishes.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["task"],
                "properties": { "task": task_schema() }
            }),
        ),
        tool_def(
            "made_stream_deliberation",
            "Run a deliberation and return every phase-transition / result frame buffered into a single response array (no live streaming over MCP stdio).",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["task"],
                "properties": { "task": task_schema() }
            }),
        ),
        tool_def(
            "made_get_deliberation_result",
            "Fetch a previously-executed deliberation by task id.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["task_id"],
                "properties": { "task_id": string_schema("Stable task id used at deliberation time.") }
            }),
        ),
        tool_def(
            "made_orchestrate",
            "Deliberate AND execute the winning proposal through the wired executor port. Returns the winner plus an execution id.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["task"],
                "properties": {
                    "task": task_schema(),
                    "execution_options": attributes_schema("Opaque executor options. Forwarded verbatim to the configured ExecutorPort.")
                }
            }),
        ),
        tool_def(
            "made_create_council",
            "Create or replace the council for a specialty. `agent_config` is opaque and passed to the agent factory.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["specialty", "num_agents"],
                "properties": {
                    "specialty": string_schema("Free-form specialty label, e.g. \"triage\"."),
                    "num_agents": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Number of agents to seat on the council."
                    },
                    "agent_config": {
                        "type": "object",
                        "additionalProperties": true,
                        "description": "Opaque config forwarded to the agent factory."
                    }
                }
            }),
        ),
        tool_def(
            "made_list_councils",
            "List the councils registered on the made.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "include_agents": {
                        "type": "boolean",
                        "description": "When true, return each council's agent roster."
                    }
                }
            }),
        ),
        tool_def(
            "made_delete_council",
            "Delete the council registered for a specialty. Idempotent: `deleted=false` means the council did not exist.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["specialty"],
                "properties": { "specialty": string_schema("Specialty whose council to delete.") }
            }),
        ),
        tool_def(
            "made_register_agent",
            "Register an agent on a council. `agent.kind` must be one supported by the wired AgentFactoryPort (e.g. noop, anthropic, openai, vllm).",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["specialty", "agent"],
                "properties": {
                    "specialty": string_schema("Specialty the agent belongs to."),
                    "agent": agent_summary_schema(),
                    "agent_config": {
                        "type": "object",
                        "additionalProperties": true,
                        "description": "Opaque per-agent factory config."
                    }
                }
            }),
        ),
        tool_def(
            "made_unregister_agent",
            "Unregister a previously-registered agent by id.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["agent_id"],
                "properties": { "agent_id": string_schema("Agent id returned by made_register_agent.") }
            }),
        ),
        tool_def(
            "made_process_trigger_event",
            "Submit a domain event that should fan out to one or more deliberations. Returns a TriggerAck reporting the dispatched task ids.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["event"],
                "properties": { "event": trigger_event_schema() }
            }),
        ),
        tool_def(
            "made_run_council_decision",
            "Run a council deliberation against a registered output contract and return the validated winner plus candidate breakdown.",
            run_council_decision_schema(),
        ),
        tool_def(
            "made_register_contract",
            "Register an `OutputContract` in the in-memory contract registry.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["contract"],
                "properties": { "contract": output_contract_schema() }
            }),
        ),
        tool_def(
            "made_list_contracts",
            "List every contract registered in the made.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        ),
        tool_def(
            "made_delete_contract",
            "Delete a registered contract by id.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["contract_id"],
                "properties": { "contract_id": string_schema("Contract id previously returned by register_contract.") }
            }),
        ),
    ]
}
