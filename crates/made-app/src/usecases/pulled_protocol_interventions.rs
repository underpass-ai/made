use super::pulled_protocol_intervention::PulledProtocolIntervention;

/// What one pull handed to one agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PulledProtocolInterventions {
    items: Vec<PulledProtocolIntervention>,
}

impl PulledProtocolInterventions {
    #[must_use]
    pub const fn new(items: Vec<PulledProtocolIntervention>) -> Self {
        Self { items }
    }

    #[must_use]
    pub fn items(&self) -> &[PulledProtocolIntervention] {
        &self.items
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
