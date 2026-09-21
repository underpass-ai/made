//! One thing a host has been handed, and the lease it holds it under.

use made_core::value_objects::HostDeliveryLease;
use time::OffsetDateTime;

use crate::services::attention::AttentionEvent;

use super::AttentionContext;

/// One attention item, leased to the host that asked for it.
///
/// The lease travels with the item because everything the host does
/// next is under it: acknowledging an intent, recording that it acted,
/// or handing it back. An item without its lease would be news the
/// host cannot answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttentionDelivery {
    lease: HostDeliveryLease,
    leased_until: OffsetDateTime,
    attention: AttentionEvent,
    context: AttentionContext,
}

impl AttentionDelivery {
    #[must_use]
    pub const fn new(
        lease: HostDeliveryLease,
        leased_until: OffsetDateTime,
        attention: AttentionEvent,
        context: AttentionContext,
    ) -> Self {
        Self {
            lease,
            leased_until,
            attention,
            context,
        }
    }

    #[must_use]
    pub const fn lease(&self) -> &HostDeliveryLease {
        &self.lease
    }

    #[must_use]
    pub const fn leased_until(&self) -> OffsetDateTime {
        self.leased_until
    }

    #[must_use]
    pub const fn attention(&self) -> &AttentionEvent {
        &self.attention
    }

    #[must_use]
    pub const fn context(&self) -> &AttentionContext {
        &self.context
    }
}
