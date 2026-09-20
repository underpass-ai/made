use crate::value_objects::HostHandoffDeclaration;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Durable external declaration; does not stop a worker, release a lease or grant takeover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostHandoffRecorded {
    pub declaration: HostHandoffDeclaration,
    #[serde(with = "time::serde::rfc3339")]
    pub recorded_at: OffsetDateTime,
}
