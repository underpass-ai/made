use serde::{Deserialize, Serialize};

use super::{SupportConfidence, SupportDecision, SupportRationale};

/// Evidence judge decision captured as domain values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportVerdict {
    decision: SupportDecision,
    confidence: SupportConfidence,
    rationale: SupportRationale,
}

impl SupportVerdict {
    #[must_use]
    pub const fn new(
        decision: SupportDecision,
        confidence: SupportConfidence,
        rationale: SupportRationale,
    ) -> Self {
        Self {
            decision,
            confidence,
            rationale,
        }
    }

    #[must_use]
    pub const fn decision(&self) -> SupportDecision {
        self.decision
    }

    #[must_use]
    pub const fn confidence(&self) -> SupportConfidence {
        self.confidence
    }

    #[must_use]
    pub const fn rationale(&self) -> &SupportRationale {
        &self.rationale
    }
}
