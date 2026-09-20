use serde::{Deserialize, Serialize};

/// Advisory paths, not authorization or a promise that a racing command will succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CeremonyPreflightAction {
    WaitForAcceptedWork,
    RecordHostHandoffEvidence,
    CompleteWithOriginalFence,
    InspectExternalEffects,
    InspectReceiptForAdoption,
    ReclaimAfterReconciliationWithNewFence,
    EnforceDeadlines,
    ResumeAdmission,
    NoFurtherWork,
}
