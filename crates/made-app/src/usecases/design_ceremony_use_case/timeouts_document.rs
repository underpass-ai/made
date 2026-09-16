use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct TimeoutsDocument {
    pub(super) step_default: u64,
}
