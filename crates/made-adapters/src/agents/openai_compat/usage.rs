use serde::Deserialize;

/// Token usage block of a Chat Completions response.
#[derive(Deserialize, Clone, Copy, Default)]
pub(in crate::agents) struct Usage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
}
