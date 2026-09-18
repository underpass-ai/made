use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::{CeremonyChildSpec, MaxChildDepth, MaxChildren};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyChildSpawn {
    children: Vec<CeremonyChildSpec>,
    max_children: MaxChildren,
    max_depth: MaxChildDepth,
}

impl CeremonyChildSpawn {
    pub fn new(
        children: Vec<CeremonyChildSpec>,
        max_children: MaxChildren,
        max_depth: MaxChildDepth,
    ) -> Result<Self, DomainError> {
        if children.is_empty() {
            return Err(DomainError::InvalidDocument {
                reason: "ceremony child spawn must declare at least one child".to_owned(),
            });
        }
        if children.len() > usize::from(max_children.get()) {
            return Err(DomainError::InvalidDocument {
                reason: "ceremony child spawn exceeds max_children".to_owned(),
            });
        }
        Ok(Self {
            children,
            max_children,
            max_depth,
        })
    }
    #[must_use]
    pub fn children(&self) -> &[CeremonyChildSpec] {
        &self.children
    }
    #[must_use]
    pub const fn max_children(&self) -> MaxChildren {
        self.max_children
    }
    #[must_use]
    pub const fn max_depth(&self) -> MaxChildDepth {
        self.max_depth
    }
}
