use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::EvidenceReference;

use super::{DeliveryNote, HostDeliveryObservationKind};

/// What a host reported about a delivery it was handed.
///
/// An observation, not a verdict: the engine records what the host said
/// and when, and the ledger's state follows from it. Nothing here is the
/// engine's own opinion about whether the work got done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostDeliveryObservation {
    kind: HostDeliveryObservationKind,
    #[serde(with = "time::serde::rfc3339")]
    observed_at: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    evidence: Option<EvidenceReference>,
    note: DeliveryNote,
}

impl HostDeliveryObservation {
    #[must_use]
    pub const fn new(
        kind: HostDeliveryObservationKind,
        observed_at: OffsetDateTime,
        evidence: Option<EvidenceReference>,
        note: DeliveryNote,
    ) -> Self {
        Self {
            kind,
            observed_at,
            evidence,
            note,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> HostDeliveryObservationKind {
        self.kind
    }

    #[must_use]
    pub const fn observed_at(&self) -> OffsetDateTime {
        self.observed_at
    }

    #[must_use]
    pub const fn evidence(&self) -> Option<&EvidenceReference> {
        self.evidence.as_ref()
    }

    #[must_use]
    pub const fn note(&self) -> &DeliveryNote {
        &self.note
    }
}
