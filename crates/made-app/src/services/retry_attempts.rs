use made_core::error::DomainError;

/// How many times a command may be decided and appended before a
/// conflict is handed back.
///
/// Counted as attempts rather than retries so the bound reads as a
/// number of tries: three attempts are one try and two retries. Never
/// zero, since a policy that tries nothing is not a policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryAttempts(u8);

impl RetryAttempts {
    /// Three attempts: enough to ride out a commuting write or two,
    /// small enough that a stream under real contention fails fast
    /// rather than spinning.
    pub const DEFAULT: Self = Self(3);

    pub fn new(attempts: u8) -> Result<Self, DomainError> {
        if attempts == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "retry_attempts",
            });
        }
        Ok(Self(attempts))
    }

    #[must_use]
    pub fn get(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_attempts_is_refused() {
        assert!(matches!(
            RetryAttempts::new(0),
            Err(DomainError::MustBeNonZero {
                field: "retry_attempts"
            })
        ));
        assert_eq!(RetryAttempts::new(2).unwrap().get(), 2);
        assert_eq!(RetryAttempts::DEFAULT.get(), 3);
    }
}
