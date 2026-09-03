use serde::Serialize;

#[derive(Serialize)]
pub(in crate::agents) struct ChatMessage<'a> {
    pub role: &'a str,
    pub content: String,
}
