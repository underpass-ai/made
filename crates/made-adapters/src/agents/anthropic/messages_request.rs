use serde::Serialize;

use super::{Message, SystemBlock};

#[derive(Serialize)]
pub(super) struct MessagesRequest<'a> {
    pub(super) model: &'a str,
    pub(super) max_tokens: u32,
    pub(super) system: Vec<SystemBlock>,
    pub(super) messages: Vec<Message<'a>>,
}
