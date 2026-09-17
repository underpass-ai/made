use serde::{Deserialize, Serialize};

/// Delivery failures already recorded at a consumer's next position.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct CeremonyEventCursorAttempt(u32);

impl CeremonyEventCursorAttempt {
    pub const NONE: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    #[must_use]
    pub const fn is_exhausted(self, max_attempts: u32) -> bool {
        self.0 >= max_attempts
    }
}
