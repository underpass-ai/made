use made_core::error::DomainError;
use made_core::value_objects::{
    ProtocolAgentExecutionId, ProtocolId, ProtocolInterventionId, ProtocolInterventionPageLimit,
    RoleId,
};

use super::intervention_resolution_filter::InterventionResolutionFilter;
use super::protocol_intervention_delivery_status::ProtocolInterventionDeliveryStatus;

/// A bounded, filtered look at one protocol's interventions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListProtocolInterventionsInput {
    pub(crate) instance_id: ProtocolId,
    pub(crate) status: Option<ProtocolInterventionDeliveryStatus>,
    pub(crate) role_id: Option<RoleId>,
    pub(crate) agent_execution_id: Option<ProtocolAgentExecutionId>,
    pub(crate) resolution: InterventionResolutionFilter,
    pub(crate) limit: ProtocolInterventionPageLimit,
    pub(crate) cursor: Option<ProtocolInterventionId>,
}

impl ListProtocolInterventionsInput {
    #[must_use]
    pub fn new(instance_id: ProtocolId) -> Self {
        Self {
            instance_id,
            status: None,
            role_id: None,
            agent_execution_id: None,
            resolution: InterventionResolutionFilter::Any,
            limit: ProtocolInterventionPageLimit::default(),
            cursor: None,
        }
    }

    /// Only items whose projected delivery status has this name.
    ///
    /// Parsed here rather than compared as a string later, so a caller
    /// that asks for a status the projection cannot produce is told so
    /// instead of quietly getting an empty page.
    pub fn in_status(mut self, status: &str) -> Result<Self, DomainError> {
        let named = ProtocolInterventionDeliveryStatus::named(status).ok_or(
            DomainError::InvariantViolated {
                reason: "unknown intervention delivery status filter",
            },
        )?;
        self.status = Some(named);
        Ok(self)
    }

    #[must_use]
    pub fn for_role(mut self, role_id: RoleId) -> Self {
        self.role_id = Some(role_id);
        self
    }

    #[must_use]
    pub fn for_agent_execution(mut self, agent_execution_id: ProtocolAgentExecutionId) -> Self {
        self.agent_execution_id = Some(agent_execution_id);
        self
    }

    #[must_use]
    pub const fn resolved(mut self, resolution: InterventionResolutionFilter) -> Self {
        self.resolution = resolution;
        self
    }

    #[must_use]
    pub const fn of_size(mut self, limit: ProtocolInterventionPageLimit) -> Self {
        self.limit = limit;
        self
    }

    #[must_use]
    pub fn after(mut self, cursor: ProtocolInterventionId) -> Self {
        self.cursor = Some(cursor);
        self
    }

    #[must_use]
    pub const fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }
}
