use serde::Serialize;

#[derive(Serialize)]
pub(super) struct ChatMessage {
    pub(super) role: &'static str,
    pub(super) content: String,
}
