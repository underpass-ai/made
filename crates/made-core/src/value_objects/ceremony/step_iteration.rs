use serde::{Deserialize, Serialize};

use crate::error::DomainError;

pub const MAX_STEP_ITERATIONS: u32 = 1_000;

/// One semantic execution of a ceremony step.
///
/// An iteration is distinct from a retry attempt: retries recover the same
/// execution after failure or lease loss, while a new iteration deliberately
/// runs successful work again because its declared stop condition is false.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepIteration(u32);

impl StepIteration {
    pub const FIRST: Self = Self(1);

    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "step_iteration",
            });
        }
        if value > MAX_STEP_ITERATIONS {
            return Err(DomainError::OutOfRange {
                field: "step_iteration",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(MAX_STEP_ITERATIONS),
            });
        }
        Ok(Self(value))
    }

    pub fn next(self) -> Result<Self, DomainError> {
        Self::new(self.0.saturating_add(1))
    }

    #[must_use]
    pub fn get(self) -> u32 {
        self.0
    }
}

impl Default for StepIteration {
    fn default() -> Self {
        Self::FIRST
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterations_start_at_one() {
        assert!(matches!(
            StepIteration::new(0),
            Err(DomainError::MustBeNonZero {
                field: "step_iteration"
            })
        ));
        assert_eq!(StepIteration::FIRST.next().unwrap().get(), 2);
        assert!(matches!(
            StepIteration::new(MAX_STEP_ITERATIONS + 1),
            Err(DomainError::OutOfRange {
                field: "step_iteration",
                ..
            })
        ));
    }
}
