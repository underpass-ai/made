use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub(super) struct ChatCompletionsRequest {
    #[serde(default)]
    pub(super) model: Option<String>,
    #[serde(default)]
    pub(super) messages: Vec<Value>,
    #[serde(default)]
    pub(super) max_tokens: Option<u32>,
}
