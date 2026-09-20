use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::CarriedEvidence;

/// What a successor started with, recorded in the successor's own
/// stream.
///
/// Appended in the same batch as the opening, so a successor cannot
/// exist without the record of what it was given — the same reason a
/// session's recollection is sealed beside its start. The evidence is
/// referenced, never copied: the predecessor's stream remains the
/// place the work happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuccessionCarried {
    pub carried: Vec<CarriedEvidence>,
    #[serde(with = "time::serde::rfc3339")]
    pub carried_at: OffsetDateTime,
}
