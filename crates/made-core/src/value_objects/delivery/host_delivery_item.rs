use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::{ProtocolId, ProtocolInterventionId};

use super::{AttentionEventId, HostDeliveryId, HostDeliveryItemKind, HostDeliveryTarget};

/// What is being delivered to a host destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostDeliveryItem {
    /// A supervisor's open question, addressed to whoever is working.
    Intervention {
        protocol_id: ProtocolId,
        intervention_id: ProtocolInterventionId,
    },
    /// A projected reason the integrator should look at the protocol.
    Attention {
        protocol_id: ProtocolId,
        attention_id: AttentionEventId,
    },
}

impl HostDeliveryItem {
    #[must_use]
    pub const fn intervention(
        protocol_id: ProtocolId,
        intervention_id: ProtocolInterventionId,
    ) -> Self {
        Self::Intervention {
            protocol_id,
            intervention_id,
        }
    }

    #[must_use]
    pub const fn attention(protocol_id: ProtocolId, attention_id: AttentionEventId) -> Self {
        Self::Attention {
            protocol_id,
            attention_id,
        }
    }

    #[must_use]
    pub const fn protocol_id(&self) -> &ProtocolId {
        match self {
            Self::Intervention { protocol_id, .. } | Self::Attention { protocol_id, .. } => {
                protocol_id
            }
        }
    }

    #[must_use]
    pub const fn kind(&self) -> HostDeliveryItemKind {
        match self {
            Self::Intervention { .. } => HostDeliveryItemKind::Intervention,
            Self::Attention { .. } => HostDeliveryItemKind::Attention,
        }
    }

    /// The identifier that distinguishes this item from its siblings.
    #[must_use]
    pub fn item_id(&self) -> &str {
        match self {
            Self::Intervention {
                intervention_id, ..
            } => intervention_id.as_str(),
            Self::Attention { attention_id, .. } => attention_id.as_str(),
        }
    }

    /// The deduplicating identity of delivering this item to `target`.
    pub fn delivery_id(&self, target: &HostDeliveryTarget) -> Result<HostDeliveryId, DomainError> {
        HostDeliveryId::derive(
            self.protocol_id(),
            self.kind(),
            self.item_id(),
            &target.target_key(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::RoleId;

    #[test]
    fn an_item_and_a_destination_name_one_delivery() {
        let item = HostDeliveryItem::intervention(
            ProtocolId::new("c-1").unwrap(),
            ProtocolInterventionId::new("i-1").unwrap(),
        );
        let target = HostDeliveryTarget::role(RoleId::new("ENGINEER").unwrap());
        assert_eq!(
            item.delivery_id(&target).unwrap().as_str(),
            "c-1:intervention:i-1:role:ENGINEER"
        );
    }
}
