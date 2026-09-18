use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use made_core::value_objects::CeremonyStepAggregation;

use super::StepRepeatDocument;

#[derive(Debug, Serialize)]
pub(super) struct StepDocument {
    pub(super) id: String,
    pub(super) state: String,
    pub(super) handler: String,
    pub(super) config: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) repeat: Option<StepRepeatDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) role_from: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) allowed_roles: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) context_writes: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) aggregate: Option<CeremonyStepAggregation>,
}
