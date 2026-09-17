use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct ContentBlock {
    #[serde(rename = "type", default)]
    pub(super) ty: String,
    #[serde(default)]
    pub(super) text: Option<String>,
}
