#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeremonyWorkerEligibility {
    Ready,
    BudgetUnavailable,
    PermissionDenied,
}
