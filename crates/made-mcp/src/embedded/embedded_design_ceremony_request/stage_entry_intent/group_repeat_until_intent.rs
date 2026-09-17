use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GroupRepeatUntilIntent {
    pub(super) step: String,
    pub(super) output_field: String,
    pub(super) equals: Value,
}
