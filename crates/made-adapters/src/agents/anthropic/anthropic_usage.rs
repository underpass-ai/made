use serde::Deserialize;

#[derive(Deserialize, Clone, Copy, Default)]
pub(super) struct AnthropicUsage {
    #[serde(default)]
    pub(super) input_tokens: u32,
    #[serde(default)]
    pub(super) output_tokens: u32,
}
