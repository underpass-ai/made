use serde::Serialize;

#[derive(Serialize)]
pub(super) struct CheckResult {
    pub(super) name: &'static str,
    pub(super) healthy: bool,
    pub(super) detail: &'static str,
}
