use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct InputsDocument {
    pub(super) required: Vec<String>,
    pub(super) optional: Vec<String>,
}
