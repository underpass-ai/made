use made_core::value_objects::CeremonyInterventionId;

use super::ceremony_intervention_view::CeremonyInterventionView;

/// One page of interventions, with the key to ask for the next.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CeremonyInterventionPage {
    entries: Vec<CeremonyInterventionView>,
    next_cursor: Option<CeremonyInterventionId>,
}

impl CeremonyInterventionPage {
    #[must_use]
    pub const fn new(
        entries: Vec<CeremonyInterventionView>,
        next_cursor: Option<CeremonyInterventionId>,
    ) -> Self {
        Self {
            entries,
            next_cursor,
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[CeremonyInterventionView] {
        &self.entries
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&CeremonyInterventionId> {
        self.next_cursor.as_ref()
    }
}
