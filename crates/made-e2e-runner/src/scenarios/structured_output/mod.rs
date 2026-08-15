mod external_context;
mod real_vllm_multi_agent;
mod report_openai;
mod report_vllm;
mod strict_schema;

pub(crate) use external_context::verify_external_context_bundle_round_trips;
pub(crate) use real_vllm_multi_agent::verify_multi_agent_council_against_real_vllm;
pub(crate) use report_openai::verify_structured_output_against_stub_llm;
pub(crate) use report_vllm::verify_structured_output_against_vllm_kind;
pub(crate) use strict_schema::verify_orchestrate_rejects_proposal_violating_json_schema;
