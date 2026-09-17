use serde::Serialize;

use super::CheckResult;

#[derive(Serialize)]
pub(super) struct ReadinessBody {
    pub(super) status: &'static str,
    pub(super) service: &'static str,
    pub(super) version: String,
    pub(super) checks: Vec<CheckResult>,
}
