use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JoinIntent {
    pub(super) condition: String,
    #[serde(default)]
    pub(super) count: Option<u32>,
}
