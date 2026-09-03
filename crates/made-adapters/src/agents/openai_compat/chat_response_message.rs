use serde::Deserialize;

#[derive(Deserialize)]
pub(in crate::agents) struct ChatResponseMessage {
    #[serde(default)]
    pub content: Option<String>,
    /// Qwen3 and other reasoning-parser-enabled models can split output
    /// between the final content and a reasoning field.
    #[serde(default)]
    pub reasoning: Option<String>,
}
