use serde::{Deserialize, Serialize};

use crate::entities::CeremonyIntervention;

/// A seat asked the table for something.
///
/// The whole intervention as it was opened: id, kind, requesting role,
/// target, request content, provenance and timestamps, with no
/// responses yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterventionRequested {
    pub intervention: CeremonyIntervention,
}
