use super::{
    BudgetMeasurement, BudgetQuantities, BudgetReservationEstimate, BudgetReservationPolicyVersion,
};
use crate::DomainError;

/// Reservation bounds are explicit in every dimension, including zero cost.
/// Unlike an absent account limit, a zero policy ceiling prohibits consumption.
#[derive(Debug, Clone)]
pub struct BudgetReservationPolicy {
    version: BudgetReservationPolicyVersion,
    ceilings: BudgetQuantities,
}

impl BudgetReservationPolicy {
    #[must_use]
    pub const fn new(version: BudgetReservationPolicyVersion, ceilings: BudgetQuantities) -> Self {
        Self { version, ceilings }
    }

    pub fn estimate(
        &self,
        version: BudgetReservationPolicyVersion,
        quantities: BudgetQuantities,
    ) -> Result<BudgetReservationEstimate, DomainError> {
        if version != self.version {
            return Err(DomainError::Conflict {
                what: "worker_budget_policy_version",
            });
        }
        for (field, value, maximum) in [
            (
                "estimated_duration_micros",
                quantities.duration().as_micros(),
                self.ceilings.duration().as_micros(),
            ),
            (
                "estimated_tokens",
                quantities.tokens().value(),
                self.ceilings.tokens().value(),
            ),
            (
                "estimated_cost_micros",
                quantities.cost().value(),
                self.ceilings.cost().value(),
            ),
            (
                "estimated_tool_calls",
                quantities.tool_calls().value(),
                self.ceilings.tool_calls().value(),
            ),
        ] {
            if value > maximum {
                return Err(DomainError::OutOfRange {
                    field,
                    value: value as f64,
                    min: 0.0,
                    max: maximum as f64,
                });
            }
        }
        Ok(BudgetReservationEstimate::new(
            BudgetMeasurement::Estimated(quantities.duration()),
            BudgetMeasurement::Estimated(quantities.tokens()),
            BudgetMeasurement::Estimated(quantities.cost()),
            BudgetMeasurement::Estimated(quantities.tool_calls()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{BudgetTokenCount, CostMicros, ExecutionDuration, ToolCallCount};

    fn quantities(cost: u64) -> BudgetQuantities {
        BudgetQuantities::new(
            ExecutionDuration::from_micros(100),
            BudgetTokenCount::new(20),
            CostMicros::new(cost),
            ToolCallCount::new(4),
        )
    }

    #[test]
    fn policy_requires_exact_version_and_preserves_zero_cost_ceiling() {
        let version = BudgetReservationPolicyVersion::new(7).unwrap();
        let policy = BudgetReservationPolicy::new(version, quantities(0));
        assert!(policy.estimate(version, quantities(0)).is_ok());
        assert!(matches!(
            policy.estimate(version, quantities(1)),
            Err(DomainError::OutOfRange { .. })
        ));
        assert!(matches!(
            policy.estimate(
                BudgetReservationPolicyVersion::new(8).unwrap(),
                quantities(0)
            ),
            Err(DomainError::Conflict { .. })
        ));
        assert!(BudgetReservationPolicyVersion::new(0).is_err());
    }
}
