use serde::Serialize;

#[derive(Serialize)]
pub(super) struct LivenessBody {
    pub(super) status: &'static str,
    pub(super) service: &'static str,
    pub(super) version: String,
}
