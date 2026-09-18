use serde::Deserialize;

use super::group_repeat_until_intent::GroupRepeatUntilIntent;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GroupRepeatIntent {
    pub(super) max_iterations: u32,
    pub(super) until: GroupRepeatUntilIntent,
}
