use crate::value_objects::HostHandoffDeclaration;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordHostHandoff {
    pub declaration: HostHandoffDeclaration,
    pub now: OffsetDateTime,
}
