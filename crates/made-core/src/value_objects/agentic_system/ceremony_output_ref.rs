use serde::{Deserialize, Serialize};

use crate::value_objects::OutputName;

use super::SystemCeremonyId;

/// One output of one composed ceremony, named from outside it.
///
/// This is how work flows between compositions without the system
/// holding the work itself: a later ceremony reads a named output of
/// an earlier one, and validation checks the pinned definition really
/// declares it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CeremonyOutputRef {
    ceremony: SystemCeremonyId,
    output: OutputName,
}

impl CeremonyOutputRef {
    #[must_use]
    pub const fn new(ceremony: SystemCeremonyId, output: OutputName) -> Self {
        Self { ceremony, output }
    }

    #[must_use]
    pub const fn ceremony(&self) -> &SystemCeremonyId {
        &self.ceremony
    }

    #[must_use]
    pub const fn output(&self) -> &OutputName {
        &self.output
    }
}
