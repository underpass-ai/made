use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::StepRepeatDocument;

#[derive(Debug, Serialize)]
pub(super) struct StepDocument {
    pub(super) id: String,
    pub(super) state: String,
    pub(super) handler: String,
    pub(super) config: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) repeat: Option<StepRepeatDocument>,
}
