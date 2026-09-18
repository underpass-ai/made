use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct TimeoutsDocument {
    pub(super) step_default: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) ceremony: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) state_default: Option<u64>,
}
