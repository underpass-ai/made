use super::RetryAttempts;

/// What a use case does when the stream moved while it was deciding.
///
/// A conflict says another writer landed first, nothing of ours did,
/// and the decision was made against a session that no longer exists.
/// Whether to decide again is the command's to say, not the store's:
/// a guard approval or a step completion commutes with what most
/// other writers do to the same session, so reloading and deciding
/// again is almost always the right answer; a transition is the one
/// move that changes which commands are legal next, so a caller who
/// lost that race should be told rather than moved on their behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictPolicy {
    /// One attempt; a conflict is returned as one.
    FailFast,
    /// Reload and decide again, up to the bound; the last conflict is
    /// returned as one.
    Retry(RetryAttempts),
}

impl ConflictPolicy {
    /// Retry with the default bound.
    #[must_use]
    pub const fn retry() -> Self {
        Self::Retry(RetryAttempts::DEFAULT)
    }

    /// How many times the command may be decided and appended.
    #[must_use]
    pub fn attempts(self) -> u8 {
        match self {
            Self::FailFast => 1,
            Self::Retry(attempts) => attempts.get(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fail_fast_is_one_attempt_and_retry_is_its_bound() {
        assert_eq!(ConflictPolicy::FailFast.attempts(), 1);
        assert_eq!(ConflictPolicy::retry().attempts(), 3);
        assert_eq!(
            ConflictPolicy::Retry(RetryAttempts::new(5).unwrap()).attempts(),
            5
        );
    }
}
