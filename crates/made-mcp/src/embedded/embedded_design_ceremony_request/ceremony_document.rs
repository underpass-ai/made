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
}
