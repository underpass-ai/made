use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::ChildCompletionRef;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildCompletionAccepted {
    pub completion: ChildCompletionRef,
    #[serde(with = "time::serde::rfc3339")]
    pub accepted_at: OffsetDateTime,
}
