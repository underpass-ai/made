use crate::value_objects::{DurationMs, IdempotencyKey};
use serde::{Deserialize, Serialize};

/// Stable identity and requested duration of one delegated heartbeat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepLeaseRenewalRequest {
    pub id: IdempotencyKey,
    pub ttl: DurationMs,
}
