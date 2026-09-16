use made_core::value_objects::{CeremonyId, StreamVersion};

/// Where to start reading a ceremony's stream, and how much of it to
/// take.
///
/// Reading is by position, as ADR-012 defines a stream: `from_version`
/// is the version already seen, so the answer starts at the record
/// after it and `StreamVersion::EMPTY` reads from the beginning. A
/// durable named cursor is a different thing and arrives as additive
/// fields with C2; it is not this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadCeremonyEventsInput {
    ceremony_id: CeremonyId,
    from_version: StreamVersion,
    limit: Option<usize>,
}

impl ReadCeremonyEventsInput {
    /// How many records an unasked-for limit takes.
    ///
    /// Big enough that a ceremony driven by hand comes back whole in
    /// one call, small enough that a caller who forgot to ask cannot
    /// pull an unbounded stream into memory.
    pub const DEFAULT_LIMIT: usize = 200;

    /// The most any one read answers with, whatever was asked for.
    /// A page is a page; a caller who wants the whole stream asks
    /// again from `next_version`.
    pub const MAX_LIMIT: usize = 1000;

    #[must_use]
    pub fn new(ceremony_id: CeremonyId, from_version: StreamVersion, limit: Option<usize>) -> Self {
        Self {
            ceremony_id,
            from_version,
            limit,
        }
    }

    #[must_use]
    pub fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }

    /// The version the caller has already seen. The read starts after
    /// it.
    #[must_use]
    pub const fn from_version(&self) -> StreamVersion {
        self.from_version
    }

    /// The limit actually applied: what was asked for, capped; the
    /// default when nothing was asked for; and the default for a zero,
    /// because "read no records" is not a question anyone means.
    #[must_use]
    pub fn limit(&self) -> usize {
        match self.limit {
            None | Some(0) => Self::DEFAULT_LIMIT,
            Some(asked) => asked.min(Self::MAX_LIMIT),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(limit: Option<usize>) -> ReadCeremonyEventsInput {
        ReadCeremonyEventsInput::new(
            CeremonyId::new("session-1").unwrap(),
            StreamVersion::EMPTY,
            limit,
        )
    }

    #[test]
    fn an_unasked_for_limit_is_the_default() {
        assert_eq!(input(None).limit(), ReadCeremonyEventsInput::DEFAULT_LIMIT);
        assert_eq!(
            input(Some(0)).limit(),
            ReadCeremonyEventsInput::DEFAULT_LIMIT
        );
    }

    #[test]
    fn a_limit_is_taken_as_asked_until_the_cap() {
        assert_eq!(input(Some(3)).limit(), 3);
        assert_eq!(
            input(Some(usize::MAX)).limit(),
            ReadCeremonyEventsInput::MAX_LIMIT
        );
    }
}
