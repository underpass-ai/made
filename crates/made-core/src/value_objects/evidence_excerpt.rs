use serde::{Deserialize, Serialize};

use super::{EvidenceBody, EvidenceReference};

/// One typed evidence excerpt presented to a support judge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceExcerpt {
    reference: EvidenceReference,
    body: EvidenceBody,
}

impl EvidenceExcerpt {
    #[must_use]
    pub const fn new(reference: EvidenceReference, body: EvidenceBody) -> Self {
        Self { reference, body }
    }

    #[must_use]
    pub const fn reference(&self) -> &EvidenceReference {
        &self.reference
    }

    #[must_use]
    pub const fn body(&self) -> &EvidenceBody {
        &self.body
    }
}
