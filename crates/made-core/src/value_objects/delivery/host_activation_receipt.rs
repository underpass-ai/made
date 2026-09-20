use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{HostActivationAdapterKind, HostTransportRef};

/// Proof that an activation adapter handed the envelope over.
///
/// Transport, not processing: an adapter that returned cleanly says the
/// host was reached, and says nothing at all about whether the host
/// looked at what it was given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostActivationReceipt {
    adapter: HostActivationAdapterKind,
    #[serde(with = "time::serde::rfc3339")]
    at: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transport_ref: Option<HostTransportRef>,
}

impl HostActivationReceipt {
    #[must_use]
    pub const fn new(
        adapter: HostActivationAdapterKind,
        at: OffsetDateTime,
        transport_ref: Option<HostTransportRef>,
    ) -> Self {
        Self {
            adapter,
            at,
            transport_ref,
        }
    }

    #[must_use]
    pub const fn adapter(&self) -> HostActivationAdapterKind {
        self.adapter
    }

    #[must_use]
    pub const fn at(&self) -> OffsetDateTime {
        self.at
    }

    #[must_use]
    pub const fn transport_ref(&self) -> Option<&HostTransportRef> {
        self.transport_ref.as_ref()
    }
}
