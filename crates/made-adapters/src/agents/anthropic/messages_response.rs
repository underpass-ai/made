use serde::Deserialize;

use super::{AnthropicUsage, ContentBlock};

#[derive(Deserialize)]
pub(super) struct MessagesResponse {
    #[serde(default)]
    pub(super) content: Vec<ContentBlock>,
    #[serde(default)]
    pub(super) usage: Option<AnthropicUsage>,
}
