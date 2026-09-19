/// Whether recording an intent opened or revisited an operation root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordExecutionIntentOutcome {
    RecordedFirst,
    RecordedAdditional,
    AlreadyRecorded,
}
