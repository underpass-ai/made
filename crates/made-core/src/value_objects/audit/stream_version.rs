use serde::{Deserialize, Serialize};

use super::AuditSequence;

/// How many events a ceremony's stream holds.
///
/// A stream that does not exist yet is at [`StreamVersion::EMPTY`];
/// after that the version is the sequence of the last record, since
/// sequences start at 1 and are contiguous. A command is decided
/// against a version and appended with it as the expectation, which is
/// what lets a store refuse a write decided against a stream that has
/// since moved on.
///
/// Not a [`super::AuditSequence`]: a sequence names a record, and there
/// is no record for an empty stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StreamVersion(u64);

impl StreamVersion {
    /// The version of a stream nothing has been appended to.
    pub const EMPTY: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The version a stream reaches once `sequence` is its last record.
    #[must_use]
    pub fn from_sequence(sequence: AuditSequence) -> Self {
        Self(sequence.value())
    }

    #[must_use]
    pub fn value(self) -> u64 {
        self.0
    }

    /// The version after one more event. Saturates rather than
    /// wrapping, as sequences do.
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self == Self::EMPTY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_stream_is_at_version_zero() {
        assert_eq!(StreamVersion::EMPTY.value(), 0);
        assert!(StreamVersion::EMPTY.is_empty());
        assert!(!StreamVersion::EMPTY.next().is_empty());
    }

    #[test]
    fn the_version_after_the_first_record_is_its_sequence() {
        assert_eq!(
            StreamVersion::from_sequence(AuditSequence::FIRST),
            StreamVersion::EMPTY.next()
        );
        assert_eq!(
            StreamVersion::from_sequence(AuditSequence::new(7).unwrap()).value(),
            7
        );
    }

    #[test]
    fn versions_order_by_length() {
        assert!(StreamVersion::EMPTY < StreamVersion::new(1));
        assert!(StreamVersion::new(2) < StreamVersion::new(10));
    }
}
