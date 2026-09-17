use serde::Serialize;

use super::CacheControl;

#[derive(Serialize)]
pub(super) struct SystemBlock {
    #[serde(rename = "type")]
    pub(super) ty: &'static str,
    pub(super) text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cache_control: Option<CacheControl>,
}
