use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::{
    GuardDocument, InputsDocument, RetryPoliciesDocument, RoleDocument, StateDocument,
    StepDocument, TimeoutsDocument, TransitionDocument,
};

#[derive(Debug, Serialize)]
pub(super) struct CeremonyDocument {
    pub(super) version: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) inputs: InputsDocument,
    pub(super) outputs: BTreeMap<String, Value>,
    pub(super) states: Vec<StateDocument>,
    pub(super) transitions: Vec<TransitionDocument>,
    pub(super) steps: Vec<StepDocument>,
    pub(super) guards: BTreeMap<String, GuardDocument>,
    pub(super) roles: Vec<RoleDocument>,
    pub(super) timeouts: TimeoutsDocument,
    pub(super) retry_policies: RetryPoliciesDocument,
    #[serde(skip_serializing_if = "is_default_max_parallel")]
    pub(super) max_parallel: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) max_transitions: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) max_bounces: Option<u32>,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip predicate receives `&T`.
const fn is_default_max_parallel(value: &u8) -> bool {
    *value == 3
}
