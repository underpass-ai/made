use std::fmt;

use serde::{Deserialize, Serialize};

/// Monotonic generation of an integrator binding within its scope.
///
/// A replaced binding raises the fence, so work carrying the old one is
/// refused instead of racing the replacement. It is the same trick the
/// step claim uses, in the one place a host can be swapped mid-ceremony.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct IntegratorFence(u64);

impl IntegratorFence {
    /// The fence a first binding is issued with.
    pub const FIRST: Self = Self(0);

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for IntegratorFence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replacement_outranks_what_it_replaced() {
        assert!(IntegratorFence::FIRST.next() > IntegratorFence::FIRST);
    }
}
