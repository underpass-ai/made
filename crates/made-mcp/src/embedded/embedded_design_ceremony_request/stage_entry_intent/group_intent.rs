use serde::Deserialize;

use super::group_repeat_intent::GroupRepeatIntent;
use super::join_intent::JoinIntent;
use crate::embedded::embedded_design_ceremony_request::StageIntent;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GroupIntent {
    #[serde(default)]
    pub(super) execution: Option<String>,
    pub(super) steps: Vec<StageIntent>,
    #[serde(default)]
    pub(super) join: Option<JoinIntent>,
    #[serde(default)]
    pub(super) repeat: Option<GroupRepeatIntent>,
}
