use serde::Serialize;

use super::ChatMessage;

#[derive(Serialize)]
pub(super) struct ChatChoice {
    pub(super) index: u32,
    pub(super) message: ChatMessage,
    pub(super) finish_reason: &'static str,
}
