use serde::{Deserialize, Serialize};

use super::{CeremonyStateKind, StateId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyState {
    id: StateId,
    kind: CeremonyStateKind,
}

impl CeremonyState {
    #[must_use]
    pub fn new(id: StateId, kind: CeremonyStateKind) -> Self {
        Self { id, kind }
    }

    #[must_use]
    pub fn initial(id: StateId) -> Self {
        Self::new(id, CeremonyStateKind::Initial)
    }

    #[must_use]
    pub fn intermediate(id: StateId) -> Self {
        Self::new(id, CeremonyStateKind::Intermediate)
    }

    #[must_use]
    pub fn terminal(id: StateId) -> Self {
        Self::new(id, CeremonyStateKind::Terminal)
    }

    #[must_use]
    pub fn id(&self) -> &StateId {
        &self.id
    }

    #[must_use]
    pub fn kind(&self) -> CeremonyStateKind {
        self.kind
    }

    #[must_use]
    pub fn is_initial(&self) -> bool {
        self.kind.is_initial()
    }

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.kind.is_terminal()
    }
}
