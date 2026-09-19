use made_adapters::workers::MetadataBudgetReservationPlanner;
use made_core::value_objects::{
    BudgetQuantities, BudgetReservationPolicy, BudgetReservationPolicyVersion, BudgetTokenCount,
    CostMicros, ExecutionDuration, ToolCallCount,
};
use made_core::DomainError;

/// Decode deployment settings and inject the domain policy into its adapter.
pub(crate) fn budget_planner_from_env() -> Result<MetadataBudgetReservationPlanner, DomainError> {
    let names = [
        "MADE_WORKER_BUDGET_POLICY_VERSION",
        "MADE_WORKER_BUDGET_MAX_DURATION_MICROS",
        "MADE_WORKER_BUDGET_MAX_TOKENS",
        "MADE_WORKER_BUDGET_MAX_COST_MICROS",
        "MADE_WORKER_BUDGET_MAX_TOOL_CALLS",
    ];
    if names.iter().all(|name| std::env::var(name).is_err()) {
        return Ok(MetadataBudgetReservationPlanner::new(None));
    }
    Ok(MetadataBudgetReservationPlanner::new(Some(
        BudgetReservationPolicy::new(
            BudgetReservationPolicyVersion::new(required(names[0])?)?,
            BudgetQuantities::new(
                ExecutionDuration::from_micros(required(names[1])?),
                BudgetTokenCount::new(required(names[2])?),
                CostMicros::new(required(names[3])?),
                ToolCallCount::new(required(names[4])?),
            ),
        ),
    )))
}

fn required(name: &'static str) -> Result<u64, DomainError> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| DomainError::InvalidDocument {
            reason: format!("{name} is required and must be non-negative"),
        })
}
