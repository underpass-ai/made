use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::{CeremonyId, CeremonyInterventionId};

use super::{AttentionEventId, HostDeliveryId, HostDeliveryItemKind, HostDeliveryTarget};

/// What is being delivered to a host destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostDeliveryItem {
    /// A supervisor's open question, addressed to whoever is working.
    Intervention {
        ceremony_id: CeremonyId,
        intervention_id: CeremonyInterventionId,
    },
    /// A projected reason the integrator should look at the ceremony.
    Attention {
        ceremony_id: CeremonyId,
        attention_id: AttentionEventId,
    },
}

impl HostDeliveryItem {
    #[must_use]
    pub const fn intervention(
        ceremony_id: CeremonyId,
        intervention_id: CeremonyInterventionId,
    ) -> Self {
        Self::Intervention {
            ceremony_id,
            intervention_id,
        }
    }

    #[must_use]
    pub const fn attention(ceremony_id: CeremonyId, attention_id: AttentionEventId) -> Self {
        Self::Attention {
            ceremony_id,
            attention_id,
        }
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> &CeremonyId {
        match self {
            Self::Intervention { ceremony_id, .. } | Self::Attention { ceremony_id, .. } => {
                ceremony_id
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
            self.ceremony_id(),
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
            CeremonyId::new("c-1").unwrap(),
            CeremonyInterventionId::new("i-1").unwrap(),
        );
        let target = HostDeliveryTarget::role(RoleId::new("ENGINEER").unwrap());
        assert_eq!(
            item.delivery_id(&target).unwrap().as_str(),
            "c-1:intervention:i-1:role:ENGINEER"
        );
    }
}
