use async_trait::async_trait;
use made_core::ports::BudgetReservationPlannerPort;
use made_core::value_objects::{
    BudgetMeasurement, BudgetReservationEstimate, BudgetReservationRequest, BudgetTokenCount,
    CostMicros, ExecutionDuration, ToolCallCount,
};
use made_core::DomainError;

/// Host-bounded planner. Estimates are explicit versioned handler metadata;
/// missing or unknown values never bypass a host ceiling.
#[derive(Debug)]
pub(crate) struct ConfiguredWorkerBudgetPlanner {
    version: u64,
    maximum_duration_micros: u64,
    maximum_tokens: u64,
    maximum_cost_micros: u64,
    maximum_tool_calls: u64,
}

impl ConfiguredWorkerBudgetPlanner {
    pub(crate) fn from_env() -> Result<Self, DomainError> {
        let names = [
            "MADE_WORKER_BUDGET_POLICY_VERSION",
            "MADE_WORKER_BUDGET_MAX_DURATION_MICROS",
            "MADE_WORKER_BUDGET_MAX_TOKENS",
            "MADE_WORKER_BUDGET_MAX_COST_MICROS",
            "MADE_WORKER_BUDGET_MAX_TOOL_CALLS",
        ];
        if names.iter().all(|name| std::env::var(name).is_err()) {
            return Ok(Self {
                version: 0,
                maximum_duration_micros: 0,
                maximum_tokens: 0,
                maximum_cost_micros: 0,
                maximum_tool_calls: 0,
            });
        }
        Ok(Self {
            version: positive("MADE_WORKER_BUDGET_POLICY_VERSION")?,
            maximum_duration_micros: required("MADE_WORKER_BUDGET_MAX_DURATION_MICROS")?,
            maximum_tokens: required("MADE_WORKER_BUDGET_MAX_TOKENS")?,
            maximum_cost_micros: required("MADE_WORKER_BUDGET_MAX_COST_MICROS")?,
            maximum_tool_calls: required("MADE_WORKER_BUDGET_MAX_TOOL_CALLS")?,
        })
    }

    fn estimate_value(
        request: &BudgetReservationRequest,
        key: &'static str,
        maximum: u64,
    ) -> Result<u64, DomainError> {
        let value = request
            .handler_config()
            .attributes()
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                invalid(format!(
                    "worker budget estimate `{key}` is required and numeric"
                ))
            })?;
        if value > maximum {
            return Err(DomainError::OutOfRange {
                field: key,
                value: value as f64,
                min: 0.0,
                max: maximum as f64,
            });
        }
        Ok(value)
    }
}

#[async_trait]
impl BudgetReservationPlannerPort for ConfiguredWorkerBudgetPlanner {
    async fn estimate(
        &self,
        request: &BudgetReservationRequest,
    ) -> Result<BudgetReservationEstimate, DomainError> {
        if self.version == 0 {
            return Err(invalid(
                "budgeted worker claims require the host budget policy settings",
            ));
        }
        let version = request
            .handler_config()
            .attributes()
            .get("budget_policy_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| invalid("worker budget policy version is required"))?;
        if version != self.version {
            return Err(DomainError::Conflict {
                what: "worker_budget_policy_version",
            });
        }
        Ok(BudgetReservationEstimate::new(
            BudgetMeasurement::Estimated(ExecutionDuration::from_micros(Self::estimate_value(
                request,
                "estimated_duration_micros",
                self.maximum_duration_micros,
            )?)),
            BudgetMeasurement::Estimated(BudgetTokenCount::new(Self::estimate_value(
                request,
                "estimated_tokens",
                self.maximum_tokens,
            )?)),
            BudgetMeasurement::Estimated(CostMicros::new(Self::estimate_value(
                request,
                "estimated_cost_micros",
                self.maximum_cost_micros,
            )?)),
            BudgetMeasurement::Estimated(ToolCallCount::new(Self::estimate_value(
                request,
                "estimated_tool_calls",
                self.maximum_tool_calls,
            )?)),
        ))
    }
}

fn required(name: &'static str) -> Result<u64, DomainError> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| invalid(format!("{name} is required and must be non-negative")))
}

fn positive(name: &'static str) -> Result<u64, DomainError> {
    required(name)?.checked_sub(1).map_or_else(
        || Err(invalid(format!("{name} must be positive"))),
        |value| Ok(value + 1),
    )
}

fn invalid(reason: impl Into<String>) -> DomainError {
    DomainError::InvalidDocument {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use made_core::value_objects::{
        Attributes, CeremonyContext, CeremonyId, StepHandlerConfig, StepHandlerKind, StepId,
    };
    use serde_json::json;

    use super::*;

    fn planner() -> ConfiguredWorkerBudgetPlanner {
        ConfiguredWorkerBudgetPlanner {
            version: 7,
            maximum_duration_micros: 100,
            maximum_tokens: 20,
            maximum_cost_micros: 30,
            maximum_tool_calls: 4,
        }
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
        let planner = ConfiguredWorkerBudgetPlanner {
            maximum_cost_micros: 0,
            ..planner()
        };
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
