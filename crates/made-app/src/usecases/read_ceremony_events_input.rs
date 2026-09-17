use super::CeremonyEventReadLimit;
use made_core::error::DomainError;
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
    limit: Option<CeremonyEventReadLimit>,
}

impl ReadCeremonyEventsInput {
    /// How many records an unasked-for limit takes.
    ///
    /// Big enough that a ceremony driven by hand comes back whole in
    /// one call, small enough that a caller who forgot to ask cannot
    /// pull an unbounded stream into memory.
    pub const DEFAULT_LIMIT: usize = 200;

    /// The most any one read answers with. A page is a page; a caller
    /// who wants the whole stream asks again from `next_version`.
    ///
    /// Asking for more is **refused**, not quietly cut down to this.
    /// The tool schema has always said `maximum: 1000` and the MCP gate
    /// refuses above it, so a caller who came through a tool was told
    /// no while a caller who spoke the RPC got a silently different
    /// page than the one they asked for — the same request, two
    /// answers, which is the whole of what parity is about. One rule:
    /// the schema, the proto comment and this all say refused.
    pub const MAX_LIMIT: usize = 1000;

    /// A limit above [`Self::MAX_LIMIT`] is the caller's to fix, so it
    /// is refused where the request is built rather than applied
    /// differently further in.
    pub fn new(
        ceremony_id: CeremonyId,
        from_version: StreamVersion,
        limit: Option<usize>,
    ) -> Result<Self, DomainError> {
        if let Some(asked) = limit {
            if asked > Self::MAX_LIMIT {
                #[allow(clippy::cast_precision_loss)] // a page size, nowhere near f64's range
                return Err(DomainError::OutOfRange {
                    field: "read_ceremony_events.limit",
                    value: asked as f64,
                    min: 0.0,
                    max: Self::MAX_LIMIT as f64,
                });
            }
        }
        Ok(Self {
            ceremony_id,
            from_version,
            limit: limit.map(CeremonyEventReadLimit::new),
        })
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

    /// The limit actually applied: what was asked for; the default when
    /// nothing was asked for; and the default for a zero, because "read
    /// no records" is not a question anyone means. Nothing is capped
    /// here — anything above the cap never became an input.
    #[must_use]
    pub fn limit(&self) -> usize {
        match self.limit.map(CeremonyEventReadLimit::get) {
            None | Some(0) => Self::DEFAULT_LIMIT,
            Some(asked) => asked,
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
        .expect("this limit is within the cap")
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
    fn a_limit_is_taken_exactly_as_asked_up_to_the_cap() {
        assert_eq!(input(Some(3)).limit(), 3);
        assert_eq!(
            input(Some(ReadCeremonyEventsInput::MAX_LIMIT)).limit(),
            ReadCeremonyEventsInput::MAX_LIMIT
        );
    }

    /// Above the cap is refused rather than cut down to it.
    ///
    /// Clamping answered a different question than the one asked and
    /// said nothing about it, and it made the engine disagree with the
    /// schema the MCP gate enforces — one request, two answers,
    /// depending on how the caller got here.
    #[test]
    fn a_limit_above_the_cap_is_refused_rather_than_clamped() {
        for asked in [ReadCeremonyEventsInput::MAX_LIMIT + 1, usize::MAX] {
            let error = ReadCeremonyEventsInput::new(
                CeremonyId::new("session-1").unwrap(),
                StreamVersion::EMPTY,
                Some(asked),
            )
            .expect_err("a page bigger than a page is not a page");
            assert!(
                matches!(
                    error,
                    DomainError::OutOfRange {
                        field: "read_ceremony_events.limit",
                        ..
                    }
                ),
                "{error:?}"
            );
        }
    }
}
