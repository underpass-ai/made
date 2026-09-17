use made_core::entities::AuditRecord;
use made_core::ports::PositionedRecord;
use serde::Serialize;

/// External delivery shape: global order beside the complete sealed record.
#[derive(Serialize)]
pub(super) struct CeremonyEventWire<'a> {
    pub global_position: u64,
    #[serde(flatten)]
    pub record: &'a AuditRecord,
}

impl<'a> From<&'a PositionedRecord> for CeremonyEventWire<'a> {
    fn from(value: &'a PositionedRecord) -> Self {
        Self {
            global_position: value.position.value(),
            record: &value.record,
        }
    }
}
