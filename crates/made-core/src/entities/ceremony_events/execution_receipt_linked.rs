use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{ExecutionReceiptLink, StepId};

/// A terminal execution receipt was consumed by one ceremony step claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReceiptLinked {
    pub step_id: StepId,
    pub link: ExecutionReceiptLink,
    #[serde(with = "time::serde::rfc3339")]
    pub linked_at: OffsetDateTime,
}
