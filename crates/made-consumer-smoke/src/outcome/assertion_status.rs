#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssertionStatus {
    Passed,
    Skipped { reason: String },
    Failed { detail: String },
}
