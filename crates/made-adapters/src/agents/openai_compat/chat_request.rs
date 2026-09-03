use serde::Serialize;

use super::ChatMessage;

#[derive(Serialize)]
pub(in crate::agents) struct ChatRequest<'a> {
    pub model: &'a str,
    pub max_tokens: u32,
    pub messages: Vec<ChatMessage<'a>>,
}
