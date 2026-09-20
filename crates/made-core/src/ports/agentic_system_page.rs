use crate::entities::AgenticSystem;
use crate::value_objects::AgenticSystemId;

/// One page of the design catalogue, in identifier order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgenticSystemPage {
    systems: Vec<AgenticSystem>,
    next_cursor: Option<AgenticSystemId>,
}

impl AgenticSystemPage {
    #[must_use]
    pub const fn new(systems: Vec<AgenticSystem>, next_cursor: Option<AgenticSystemId>) -> Self {
        Self {
            systems,
            next_cursor,
        }
    }

    #[must_use]
    pub fn systems(&self) -> &[AgenticSystem] {
        &self.systems
    }

    #[must_use]
    pub fn into_systems(self) -> Vec<AgenticSystem> {
        self.systems
    }

    /// Where to continue, when there is more.
    #[must_use]
    pub const fn next_cursor(&self) -> Option<&AgenticSystemId> {
        self.next_cursor.as_ref()
    }
}
