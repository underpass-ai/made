use serde::{Deserialize, Serialize};

use super::{SourceRecordRef, StepId, StepOutput};

/// One completed step of a predecessor, carried into a successor step.
///
/// The output travels because a successor that could not read what was
/// decided would be starting over; the source travels because a step
/// the successor never ran must never read as though it had. Both
/// halves are the point: evidence without provenance is a copy, and
/// provenance without the output is a footnote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarriedEvidence {
    successor_step_id: StepId,
    source: SourceRecordRef,
    output: StepOutput,
}

impl CarriedEvidence {
    #[must_use]
    pub const fn new(successor_step_id: StepId, source: SourceRecordRef, output: StepOutput) -> Self {
        Self {
            successor_step_id,
            source,
            output,
        }
    }

    #[must_use]
    pub const fn successor_step_id(&self) -> &StepId {
        &self.successor_step_id
    }

    #[must_use]
    pub const fn source(&self) -> &SourceRecordRef {
        &self.source
    }

    #[must_use]
    pub const fn output(&self) -> &StepOutput {
        &self.output
    }
}
