use serde::{Deserialize, Serialize};

use crate::value_objects::IdempotencyKey;

use super::ProcessedActionKind;

/// The act that closed a delivery, named so a repeat is recognisable.
///
/// Equality is what makes `mark_processed` idempotent: the same act,
/// offered twice by a host that retried after losing its answer, is one
/// act. A different one on a delivery that is already processed is a
/// conflict, not a second closing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessedActionRef {
    kind: ProcessedActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    idempotency_key: Option<IdempotencyKey>,
}

impl ProcessedActionRef {
    #[must_use]
    pub const fn new(kind: ProcessedActionKind, idempotency_key: Option<IdempotencyKey>) -> Self {
        Self {
            kind,
            idempotency_key,
        }
    }

    /// An act with no key of its own; the kind is the whole identity.
    #[must_use]
    pub const fn of(kind: ProcessedActionKind) -> Self {
        Self::new(kind, None)
    }

    #[must_use]
    pub const fn kind(&self) -> ProcessedActionKind {
        self.kind
    }

    #[must_use]
    pub const fn idempotency_key(&self) -> Option<&IdempotencyKey> {
        self.idempotency_key.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_act_offered_twice_is_one_act() {
        assert_eq!(
            ProcessedActionRef::of(ProcessedActionKind::Responded),
            ProcessedActionRef::of(ProcessedActionKind::Responded)
        );
        assert_ne!(
            ProcessedActionRef::of(ProcessedActionKind::Responded),
            ProcessedActionRef::of(ProcessedActionKind::NoAction)
        );
    }
}
