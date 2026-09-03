use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct RetryPolicyDocument {
    pub(super) max_attempts: u32,
    pub(super) backoff_seconds: u64,
}
