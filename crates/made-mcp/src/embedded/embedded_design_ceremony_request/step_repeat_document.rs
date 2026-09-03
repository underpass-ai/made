use serde::Serialize;

use super::RepeatUntilDocument;

#[derive(Debug, Serialize)]
pub(super) struct StepRepeatDocument {
    pub(super) max_iterations: u32,
    pub(super) until: RepeatUntilDocument,
}
