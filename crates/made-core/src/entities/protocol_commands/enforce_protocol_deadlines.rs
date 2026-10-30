use time::OffsetDateTime;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnforceProtocolDeadlines {
    pub now: OffsetDateTime,
}
