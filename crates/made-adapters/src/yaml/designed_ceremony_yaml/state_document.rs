use made_core::value_objects::StateExecution;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct StateDocument {
    pub(super) id: String,
    #[serde(skip_serializing_if = "is_false")]
    pub(super) initial: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub(super) terminal: bool,
    #[serde(skip_serializing_if = "StateExecution::is_sequential")]
    pub(super) execution: StateExecution,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(value: &bool) -> bool {
    !*value
}
