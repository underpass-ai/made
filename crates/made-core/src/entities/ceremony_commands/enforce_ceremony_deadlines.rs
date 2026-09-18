use time::OffsetDateTime;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnforceCeremonyDeadlines {
    pub now: OffsetDateTime,
}
