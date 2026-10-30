use time::OffsetDateTime;

use crate::value_objects::ChildCompletionRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptChildCompletion {
    pub completion: ChildCompletionRef,
    pub now: OffsetDateTime,
}
