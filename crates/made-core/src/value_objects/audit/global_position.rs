use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Where a sealed record sits in the order every stream shares.
///
/// Positions start at 1 and strictly increase across all streams in the
/// order the store accepted the appends; the store assigns them inside
/// the transaction that lands the records, so two appends can never
/// claim the same one. A consumer that reads every stream — a
/// publisher, a projection — keeps a position as its cursor instead of
/// one sequence per ceremony.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GlobalPosition(u64);

impl GlobalPosition {
    /// The position of the first record ever appended.
    pub const FIRST: Self = Self(1);

    pub fn new(value: u64) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "global_position",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> u64 {
        self.0
    }

    /// The position the next record will take. Saturates rather than
    /// wrapping: a wrapped position would let a cursor replay the
    /// beginning.
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_rejected() {
        assert!(matches!(
            GlobalPosition::new(0),
            Err(DomainError::MustBeNonZero {
                field: "global_position"
            })
        ));
    }

    #[test]
    fn the_first_position_is_one_and_positions_advance_by_one() {
        assert_eq!(GlobalPosition::FIRST.value(), 1);
        assert_eq!(GlobalPosition::FIRST.next().value(), 2);
        assert!(GlobalPosition::FIRST < GlobalPosition::FIRST.next());
    }
}
