use super::pulled_ceremony_intervention::PulledCeremonyIntervention;

/// What one pull handed to one agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PulledCeremonyInterventions {
    items: Vec<PulledCeremonyIntervention>,
}

impl PulledCeremonyInterventions {
    #[must_use]
    pub const fn new(items: Vec<PulledCeremonyIntervention>) -> Self {
        Self { items }
    }

    #[must_use]
    pub fn items(&self) -> &[PulledCeremonyIntervention] {
        &self.items
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
