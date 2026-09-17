use serde::Serialize;

use super::{ChatChoice, ChatUsage};

#[derive(Serialize)]
pub(super) struct ChatCompletionsResponse {
    pub(super) id: &'static str,
    pub(super) object: &'static str,
    pub(super) created: u64,
    pub(super) model: String,
    pub(super) choices: Vec<ChatChoice>,
    pub(super) usage: ChatUsage,
}
