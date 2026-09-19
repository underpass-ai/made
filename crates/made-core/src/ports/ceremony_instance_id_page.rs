use crate::value_objects::CeremonyId;

/// One bounded, ordered page from the authoritative stream index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyInstanceIdPage {
    ids: Vec<CeremonyId>,
    has_more: bool,
}

impl CeremonyInstanceIdPage {
    #[must_use]
    pub fn new(ids: Vec<CeremonyId>, has_more: bool) -> Self {
        Self { ids, has_more }
    }

    #[must_use]
    pub fn ids(&self) -> &[CeremonyId] {
        &self.ids
    }

    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}
