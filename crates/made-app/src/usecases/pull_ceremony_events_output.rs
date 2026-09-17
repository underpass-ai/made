use made_core::ports::PositionedRecord;
use made_core::value_objects::GlobalPosition;

/// Positioned records and the durable progress visible after this request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullCeremonyEventsOutput {
    records: Vec<PositionedRecord>,
    acknowledged_through: Option<GlobalPosition>,
}

impl PullCeremonyEventsOutput {
    #[must_use]
    pub fn new(
        records: Vec<PositionedRecord>,
        acknowledged_through: Option<GlobalPosition>,
    ) -> Self {
        Self {
            records,
            acknowledged_through,
        }
    }

    #[must_use]
    pub fn records(&self) -> &[PositionedRecord] {
        &self.records
    }

    #[must_use]
    pub fn into_records(self) -> Vec<PositionedRecord> {
        self.records
    }

    #[must_use]
    pub const fn acknowledged_through(&self) -> Option<GlobalPosition> {
        self.acknowledged_through
    }
}
