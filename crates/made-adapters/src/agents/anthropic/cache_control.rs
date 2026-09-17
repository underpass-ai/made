use serde::Serialize;

#[derive(Serialize)]
pub(super) struct CacheControl {
    #[serde(rename = "type")]
    pub(super) ty: &'static str,
}
