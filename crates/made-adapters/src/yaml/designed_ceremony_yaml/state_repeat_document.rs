use serde::Serialize;

use super::StateRepeatUntilDocument;

#[derive(Debug, Serialize)]
pub(super) struct StateRepeatDocument {
    pub(super) max_iterations: u32,
    pub(super) until: StateRepeatUntilDocument,
}
