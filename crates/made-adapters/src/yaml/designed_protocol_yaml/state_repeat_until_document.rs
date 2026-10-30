use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub(super) struct StateRepeatUntilDocument {
    pub(super) step: String,
    pub(super) output_field: String,
    pub(super) equals: Value,
}
