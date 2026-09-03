use time::OffsetDateTime;

#[derive(Debug, Clone, Copy)]
pub(in crate::usecases) struct FixedClock {
    pub(super) now: OffsetDateTime,
}
