use serde::Serialize;

#[derive(Serialize)]
pub(super) struct Message<'a> {
    pub(super) role: &'a str,
    pub(super) content: String,
}
