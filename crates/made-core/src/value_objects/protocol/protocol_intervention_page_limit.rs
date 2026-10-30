use std::fmt;

use crate::error::DomainError;

const MIN: usize = 1;
const MAX: usize = 100;

/// How many interventions one page of a listing may carry.
///
/// Bounded at both ends, like every other page limit here: a page of
/// none makes no progress, and an unbounded one turns one read of a
/// long-running protocol into an unbounded walk of its items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProtocolInterventionPageLimit(usize);

impl ProtocolInterventionPageLimit {
    pub const MAX: Self = Self(MAX);

    pub fn new(value: usize) -> Result<Self, DomainError> {
        if !(MIN..=MAX).contains(&value) {
            return Err(DomainError::OutOfRange {
                field: "protocol_intervention_page_limit",
                value: value as f64,
                min: MIN as f64,
                max: MAX as f64,
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

impl Default for ProtocolInterventionPageLimit {
    fn default() -> Self {
        Self::MAX
    }
}

impl fmt::Display for ProtocolInterventionPageLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_of_none_and_a_page_of_a_thousand_are_both_refused() {
        assert!(ProtocolInterventionPageLimit::new(0).is_err());
        assert!(ProtocolInterventionPageLimit::new(101).is_err());
        assert_eq!(ProtocolInterventionPageLimit::default().value(), 100);
    }
}
