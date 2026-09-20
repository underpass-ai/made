use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::HostAgentIncarnation;

use super::{HostDeliveryId, HostDeliveryLeaseId};

/// Exclusive, expiring right to deliver one item to one host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostDeliveryLease {
    delivery_id: HostDeliveryId,
    lease_id: HostDeliveryLeaseId,
    owner: HostAgentIncarnation,
    #[serde(with = "time::serde::rfc3339")]
    leased_until: OffsetDateTime,
}

impl HostDeliveryLease {
    #[must_use]
    pub const fn new(
        delivery_id: HostDeliveryId,
        lease_id: HostDeliveryLeaseId,
        owner: HostAgentIncarnation,
        leased_until: OffsetDateTime,
    ) -> Self {
        Self {
            delivery_id,
            lease_id,
            owner,
            leased_until,
        }
    }

    #[must_use]
    pub const fn delivery_id(&self) -> &HostDeliveryId {
        &self.delivery_id
    }

    /// The fencing token: a replaced holder cannot commit or clear.
    #[must_use]
    pub const fn lease_id(&self) -> &HostDeliveryLeaseId {
        &self.lease_id
    }

    #[must_use]
    pub const fn owner(&self) -> &HostAgentIncarnation {
        &self.owner
    }

    #[must_use]
    pub const fn leased_until(&self) -> OffsetDateTime {
        self.leased_until
    }

    /// Whether this lease still excludes another holder at `now`.
    #[must_use]
    pub fn is_live_at(&self, now: OffsetDateTime) -> bool {
        now < self.leased_until
    }
}
