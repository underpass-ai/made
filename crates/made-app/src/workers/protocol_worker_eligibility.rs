#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolWorkerEligibility {
    Ready,
    BudgetUnavailable,
    PermissionDenied,
}
