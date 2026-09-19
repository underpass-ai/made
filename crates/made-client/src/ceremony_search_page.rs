use made_proto::v1::CeremonyInstanceState;

/// One bounded page from the public ceremony search API.
#[derive(Clone, Debug, PartialEq)]
pub struct CeremonySearchPage {
    instances: Vec<CeremonyInstanceState>,
    next_cursor: Option<String>,
}

impl CeremonySearchPage {
    pub(crate) fn new(instances: Vec<CeremonyInstanceState>, next_cursor: Option<String>) -> Self {
        Self {
            instances,
            next_cursor,
        }
    }

    #[must_use]
    pub fn instances(&self) -> &[CeremonyInstanceState] {
        &self.instances
    }

    #[must_use]
    pub fn next_cursor(&self) -> Option<&str> {
        self.next_cursor.as_deref()
    }
}
