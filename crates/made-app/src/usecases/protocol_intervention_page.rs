use made_core::value_objects::ProtocolInterventionId;

use super::protocol_intervention_view::ProtocolInterventionView;

/// One page of interventions, with the key to ask for the next.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProtocolInterventionPage {
    entries: Vec<ProtocolInterventionView>,
    next_cursor: Option<ProtocolInterventionId>,
}

impl ProtocolInterventionPage {
    #[must_use]
    pub const fn new(
        entries: Vec<ProtocolInterventionView>,
        next_cursor: Option<ProtocolInterventionId>,
    ) -> Self {
        Self {
            entries,
            next_cursor,
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[ProtocolInterventionView] {
        &self.entries
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&ProtocolInterventionId> {
        self.next_cursor.as_ref()
    }
}
