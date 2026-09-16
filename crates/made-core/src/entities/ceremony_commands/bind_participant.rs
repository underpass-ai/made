use time::OffsetDateTime;

use crate::value_objects::{RoleId, Specialty};

/// Seat a role for this session, replacing whoever held it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindParticipant {
    pub role_id: RoleId,
    pub specialty: Specialty,
    pub now: OffsetDateTime,
}
