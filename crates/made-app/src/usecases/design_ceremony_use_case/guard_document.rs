use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct GuardDocument {
    #[serde(rename = "type")]
    pub(super) guard_type: String,
    pub(super) check: String,
}
