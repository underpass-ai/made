use crate::value_objects::ProtocolId;

/// One bounded, ordered page from the authoritative stream index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolInstanceIdPage {
    ids: Vec<ProtocolId>,
    has_more: bool,
}

impl ProtocolInstanceIdPage {
    #[must_use]
    pub fn new(ids: Vec<ProtocolId>, has_more: bool) -> Self {
        Self { ids, has_more }
    }

    #[must_use]
    pub fn ids(&self) -> &[ProtocolId] {
        &self.ids
    }

    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}
