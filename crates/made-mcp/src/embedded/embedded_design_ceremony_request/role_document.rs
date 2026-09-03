use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct RoleDocument {
    pub(super) id: String,
    pub(super) allowed_actions: Vec<String>,
}
