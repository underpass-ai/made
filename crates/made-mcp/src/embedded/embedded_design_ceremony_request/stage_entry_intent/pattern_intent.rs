use serde::Deserialize;

use super::join_intent::JoinIntent;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PatternIntent {
    pub(super) kind: String,
    pub(super) roles: Vec<String>,
    pub(super) instructions: String,
    #[serde(default)]
    pub(super) manager_role_id: Option<String>,
    #[serde(default)]
    pub(super) max_iterations: Option<u32>,
    #[serde(default)]
    pub(super) fallback_role_id: Option<String>,
    #[serde(default)]
    pub(super) join: Option<JoinIntent>,
}
