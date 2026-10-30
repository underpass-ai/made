//! Provenance for an intervention selected from an earlier table response.

use serde::{Deserialize, Serialize};

use super::{ProtocolInterventionId, RoleId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolInterventionProvenance {
    #[serde(rename = "source_intervention_id")]
    source_intervention: ProtocolInterventionId,
    #[serde(rename = "source_response_role_id")]
    source_response_role: RoleId,
    #[serde(rename = "selected_role_id")]
    selected_role: RoleId,
}

impl ProtocolInterventionProvenance {
    #[must_use]
    pub fn selected_from(
        source_intervention_id: ProtocolInterventionId,
        source_response_role_id: RoleId,
        selected_role_id: RoleId,
    ) -> Self {
        Self {
            source_intervention: source_intervention_id,
            source_response_role: source_response_role_id,
            selected_role: selected_role_id,
        }
    }

    #[must_use]
    pub fn source_intervention_id(&self) -> &ProtocolInterventionId {
        &self.source_intervention
    }

    #[must_use]
    pub fn source_response_role_id(&self) -> &RoleId {
        &self.source_response_role
    }

    #[must_use]
    pub fn selected_role_id(&self) -> &RoleId {
        &self.selected_role
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn serialized_shape_keeps_explicit_domain_identifiers() {
        let provenance = ProtocolInterventionProvenance::selected_from(
            ProtocolInterventionId::new("table-opinion").unwrap(),
            RoleId::new("OBSERVER").unwrap(),
            RoleId::new("QUEUE_SPECIALIST").unwrap(),
        );

        assert_eq!(
            serde_json::to_value(provenance).unwrap(),
            json!({
                "source_intervention_id": "table-opinion",
                "source_response_role_id": "OBSERVER",
                "selected_role_id": "QUEUE_SPECIALIST",
            })
        );
    }
}
