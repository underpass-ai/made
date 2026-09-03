use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct TransitionDocument {
    pub(super) from: String,
    pub(super) to: String,
    pub(super) trigger: String,
    pub(super) guards: Vec<String>,
}
