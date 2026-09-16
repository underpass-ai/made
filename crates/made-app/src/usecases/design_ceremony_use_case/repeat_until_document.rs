use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub(super) struct RepeatUntilDocument {
    pub(super) output_field: String,
    pub(super) equals: Value,
}
