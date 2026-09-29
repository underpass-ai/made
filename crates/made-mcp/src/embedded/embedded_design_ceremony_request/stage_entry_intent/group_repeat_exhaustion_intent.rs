use serde::Deserialize;

/// Where a group goes when its repeat is exhausted.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GroupRepeatExhaustionIntent {
    pub(super) terminal: String,
}
