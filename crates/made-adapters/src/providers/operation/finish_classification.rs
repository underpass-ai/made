use super::{FinishReason, ProviderErrorClassification};
/// Finish reason or low-cardinality provider failure classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishClassification {
    Finished(FinishReason),
    Error(ProviderErrorClassification),
}
