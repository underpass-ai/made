use async_trait::async_trait;
use made_core::ports::BudgetReservationPlannerPort;
use made_core::value_objects::{
    BudgetQuantities, BudgetReservationEstimate, BudgetReservationPolicy,
    BudgetReservationPolicyVersion, BudgetReservationRequest, BudgetTokenCount, CostMicros,
    ExecutionDuration, ToolCallCount,
};
use made_core::DomainError;

/// Maps handler metadata into the typed domain policy; never reads host config.
#[derive(Debug)]
pub struct MetadataBudgetReservationPlanner {
    policy: Option<BudgetReservationPolicy>,
}

impl MetadataBudgetReservationPlanner {
    #[must_use]
    pub const fn new(policy: Option<BudgetReservationPolicy>) -> Self {
        Self { policy }
    }
}

#[async_trait]
impl BudgetReservationPlannerPort for MetadataBudgetReservationPlanner {
    async fn estimate(
        &self,
        request: &BudgetReservationRequest,
    ) -> Result<BudgetReservationEstimate, DomainError> {
        let policy = self
            .policy
            .as_ref()
            .ok_or_else(|| DomainError::InvalidDocument {
                reason: "budgeted worker claims require the host budget policy settings".into(),
            })?;
        policy.estimate(
            BudgetReservationPolicyVersion::new(value(request, "budget_policy_version")?)?,
            BudgetQuantities::new(
                ExecutionDuration::from_micros(value(request, "estimated_duration_micros")?),
                BudgetTokenCount::new(value(request, "estimated_tokens")?),
                CostMicros::new(value(request, "estimated_cost_micros")?),
                ToolCallCount::new(value(request, "estimated_tool_calls")?),
            ),
        )
    }
}

fn value(request: &BudgetReservationRequest, key: &'static str) -> Result<u64, DomainError> {
    request
        .handler_config()
        .attributes()
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| DomainError::InvalidDocument {
            reason: format!("worker budget estimate `{key}` is required and numeric"),
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use made_core::value_objects::{
        Attributes, BudgetMeasurement, CeremonyContext, CeremonyId, StepHandlerConfig,
        StepHandlerKind, StepId,
    };
    use serde_json::json;

    use super::*;

    fn planner() -> MetadataBudgetReservationPlanner {
        MetadataBudgetReservationPlanner::new(Some(BudgetReservationPolicy::new(
            BudgetReservationPolicyVersion::new(7).unwrap(),
            BudgetQuantities::new(
                ExecutionDuration::from_micros(100),
                BudgetTokenCount::new(20),
                CostMicros::new(30),
                ToolCallCount::new(4),
            ),
        )))
    }

    fn request(entries: [(&str, serde_json::Value); 5]) -> BudgetReservationRequest {
        let attributes = Attributes::new(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect::<BTreeMap<_, _>>(),
        )
        .unwrap();
        BudgetReservationRequest::new(
            CeremonyId::new("budgeted-worker").unwrap(),
            StepId::new("work").unwrap(),
            StepHandlerKind::new("oci").unwrap(),
            StepHandlerConfig::new(attributes),
            CeremonyContext::empty(),
        )
    }

    #[tokio::test]
    async fn exact_versioned_estimates_within_host_ceilings_are_accepted() {
        let estimate = planner()
            .estimate(&request([
                ("budget_policy_version", json!(7)),
                ("estimated_duration_micros", json!(80)),
                ("estimated_tokens", json!(20)),
                ("estimated_cost_micros", json!(25)),
                ("estimated_tool_calls", json!(2)),
            ]))
            .await
            .unwrap();

        assert_eq!(
            estimate.tokens(),
            BudgetMeasurement::Estimated(BudgetTokenCount::new(20))
        );
    }

    #[tokio::test]
    async fn unknown_or_over_ceiling_estimates_fail_closed() {
        let unknown = planner()
            .estimate(&request([
                ("budget_policy_version", json!(7)),
                ("estimated_duration_micros", json!(80)),
                ("estimated_tokens", json!("unknown")),
                ("estimated_cost_micros", json!(25)),
                ("estimated_tool_calls", json!(2)),
            ]))
            .await;
        assert!(unknown.is_err());

        let over = planner()
            .estimate(&request([
                ("budget_policy_version", json!(7)),
                ("estimated_duration_micros", json!(101)),
                ("estimated_tokens", json!(20)),
                ("estimated_cost_micros", json!(25)),
                ("estimated_tool_calls", json!(2)),
            ]))
            .await;
        assert!(matches!(over, Err(DomainError::OutOfRange { .. })));
    }

    #[tokio::test]
    async fn zero_external_cost_ceiling_accepts_only_zero_cost() {
        let planner = MetadataBudgetReservationPlanner::new(Some(BudgetReservationPolicy::new(
            BudgetReservationPolicyVersion::new(7).unwrap(),
            BudgetQuantities::new(
                ExecutionDuration::from_micros(100),
                BudgetTokenCount::new(20),
                CostMicros::new(0),
                ToolCallCount::new(4),
            ),
        )));
        let zero = planner
            .estimate(&request([
                ("budget_policy_version", json!(7)),
                ("estimated_duration_micros", json!(80)),
                ("estimated_tokens", json!(20)),
                ("estimated_cost_micros", json!(0)),
                ("estimated_tool_calls", json!(2)),
            ]))
            .await;
        assert!(zero.is_ok());

        let paid = planner
            .estimate(&request([
                ("budget_policy_version", json!(7)),
                ("estimated_duration_micros", json!(80)),
                ("estimated_tokens", json!(20)),
                ("estimated_cost_micros", json!(1)),
                ("estimated_tool_calls", json!(2)),
            ]))
            .await;
        assert!(matches!(paid, Err(DomainError::OutOfRange { .. })));
    }
}
