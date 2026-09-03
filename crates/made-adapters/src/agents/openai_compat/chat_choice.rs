use serde::Deserialize;

use super::chat_response_message::ChatResponseMessage;

#[derive(Deserialize)]
pub(in crate::agents) struct ChatChoice {
    #[serde(default)]
    pub message: Option<ChatResponseMessage>,
}
