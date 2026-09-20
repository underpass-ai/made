use serde::{Deserialize, Serialize};

use super::{CeremonyId, DefinitionPin, IdempotencyKey};
use crate::value_objects::{AuditRecordHash, StreamVersion};

/// Which ceremony this one succeeds, as the successor's own opening
/// records it.
///
/// The predecessor's head and version are part of the reference: a
/// successor says which cut of the predecessor it was planned against,
/// so "what had happened when this was decided" is answerable from the
/// successor's first record instead of by guessing at timestamps. Both
/// pins travel because the whole reason a successor exists is that the
/// definition changed, and a handoff that did not record both sides of
/// that change would lose the fact it was made for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonySuccession {
    predecessor_id: CeremonyId,
    predecessor_head: AuditRecordHash,
    predecessor_version: StreamVersion,
    predecessor_definition: DefinitionPin,
    successor_definition: DefinitionPin,
    plan_id: IdempotencyKey,
}

impl CeremonySuccession {
    #[must_use]
    pub const fn new(
        predecessor_id: CeremonyId,
        predecessor_head: AuditRecordHash,
        predecessor_version: StreamVersion,
        predecessor_definition: DefinitionPin,
        successor_definition: DefinitionPin,
        plan_id: IdempotencyKey,
    ) -> Self {
        Self {
            predecessor_id,
            predecessor_head,
            predecessor_version,
            predecessor_definition,
            successor_definition,
            plan_id,
        }
    }

    #[must_use]
    pub const fn predecessor_id(&self) -> &CeremonyId {
        &self.predecessor_id
    }

    #[must_use]
    pub const fn predecessor_head(&self) -> AuditRecordHash {
        self.predecessor_head
    }

    #[must_use]
    pub const fn predecessor_version(&self) -> StreamVersion {
        self.predecessor_version
    }

    #[must_use]
    pub const fn predecessor_definition(&self) -> &DefinitionPin {
        &self.predecessor_definition
    }

    #[must_use]
    pub const fn successor_definition(&self) -> &DefinitionPin {
        &self.successor_definition
    }

    #[must_use]
    pub const fn plan_id(&self) -> &IdempotencyKey {
        &self.plan_id
    }
}
