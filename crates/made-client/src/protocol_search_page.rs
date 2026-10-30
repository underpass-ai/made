use made_proto::v1::ProtocolInstanceState;

/// One bounded page from the public protocol search API.
#[derive(Clone, Debug, PartialEq)]
pub struct ProtocolSearchPage {
    instances: Vec<ProtocolInstanceState>,
    next_cursor: Option<String>,
}

impl ProtocolSearchPage {
    pub(crate) fn new(instances: Vec<ProtocolInstanceState>, next_cursor: Option<String>) -> Self {
        Self {
            instances,
            next_cursor,
        }
    }

    #[must_use]
    pub fn instances(&self) -> &[ProtocolInstanceState] {
        &self.instances
    }

    #[must_use]
    pub fn next_cursor(&self) -> Option<&str> {
        self.next_cursor.as_deref()
    }
}
