use serde::Deserialize;

use super::{chat_choice::ChatChoice, usage::Usage};

#[derive(Deserialize)]
pub(in crate::agents) struct ChatResponse {
    #[serde(default)]
    pub choices: Vec<ChatChoice>,
    /// Token accounting can be absent on streamed or error-shaped bodies.
    #[serde(default)]
    pub usage: Option<Usage>,
}
