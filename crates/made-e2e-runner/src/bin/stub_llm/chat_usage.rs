use serde::Serialize;

#[allow(clippy::struct_field_names)]
#[derive(Serialize)]
pub(super) struct ChatUsage {
    pub(super) prompt_tokens: u32,
    pub(super) completion_tokens: u32,
    pub(super) total_tokens: u32,
}
