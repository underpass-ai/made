use std::fmt;

use serde::{Deserialize, Serialize};

/// How many times a delivery has been handed out and come back failed.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct DeliveryAttempt(u32);

impl DeliveryAttempt {
    /// A delivery that has not been attempted yet.
    pub const NONE: Self = Self(0);

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    /// One more attempt, saturating rather than wrapping: a counter that
    /// wrapped would hand an exhausted delivery back to a host forever.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for DeliveryAttempt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counting_saturates_instead_of_wrapping() {
        assert_eq!(DeliveryAttempt::NONE.next().value(), 1);
        assert_eq!(DeliveryAttempt(u32::MAX).next().value(), u32::MAX);
    }
}
