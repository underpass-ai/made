use std::fmt;

use crate::error::DomainError;

const MIN: u32 = 1;
const MAX: u32 = 100;

/// How many deliveries one call may hand back.
///
/// Capped in the domain rather than at each surface, so no caller can
/// ask a host to swallow the whole ledger in one answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostDeliveryPageLimit(u32);

impl HostDeliveryPageLimit {
    /// The largest page the ledger will build.
    pub const MAX: Self = Self(MAX);

    /// Construct a validated limit.
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if !(MIN..=MAX).contains(&value) {
            return Err(DomainError::OutOfRange {
                field: "host_delivery_page_limit",
                value: f64::from(value),
                min: f64::from(MIN),
                max: f64::from(MAX),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl Default for HostDeliveryPageLimit {
    fn default() -> Self {
        Self::MAX
    }
}

impl fmt::Display for HostDeliveryPageLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_of_none_and_a_page_of_a_thousand_are_both_refused() {
        assert!(HostDeliveryPageLimit::new(0).is_err());
        assert!(HostDeliveryPageLimit::new(101).is_err());
        assert_eq!(HostDeliveryPageLimit::default().value(), 100);
    }
}
