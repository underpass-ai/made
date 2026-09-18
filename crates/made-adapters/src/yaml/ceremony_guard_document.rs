use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyGuard, ChildJoin, ChildQuorum, ChildrenCompletedCondition, GuardCondition, GuardName,
    JoinStepCount, OutputFieldGuardCondition, StepId, StepOutputField,
    StepRepeatExhaustedGuardCondition, StepStatus,
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CeremonyGuardDocument {
    #[serde(rename = "type")]
    guard_type: String,
    check: String,
}

impl CeremonyGuardDocument {
    pub(super) fn into_domain(self, name: String) -> Result<CeremonyGuard, DomainError> {
        let condition = self.into_condition()?;
        Ok(CeremonyGuard::new(GuardName::new(name)?, condition))
    }

    fn into_condition(self) -> Result<GuardCondition, DomainError> {
        if self.guard_type == "human" || self.check == "manual_approval" {
            return Ok(GuardCondition::HumanApproval);
        }
        if self.check == "all_steps_completed" {
            return Ok(GuardCondition::AllStepsCompleted);
        }
        if self.check == "any_step_completed" {
            return Ok(GuardCondition::AnyStepCompleted);
        }
        if let Some(count) = self.check.strip_prefix("steps_completed:") {
            let count = count.parse::<u32>().map_err(|_| unsupported_guard())?;
            return Ok(GuardCondition::StepsCompleted(JoinStepCount::new(count)?));
        }
        if self.check.starts_with("output_field:") {
            return parse_output_field_guard(&self.check);
        }
        if let Some(step) = self.check.strip_prefix("step_repeat_exhausted:") {
            return Ok(GuardCondition::StepRepeatExhausted(
                StepRepeatExhaustedGuardCondition::new(StepId::new(step)?),
            ));
        }
        if let Some(body) = self.check.strip_prefix("children_completed:") {
            return parse_children_completed(body);
        }
        parse_step_status_guard(&self.check)
    }
}

fn parse_children_completed(body: &str) -> Result<GuardCondition, DomainError> {
    let mut parts = body.split(':');
    let step_id = StepId::new(parts.next().ok_or_else(unsupported_guard)?)?;
    let join = match (parts.next(), parts.next(), parts.next()) {
        (Some("all"), None, None) => ChildJoin::All,
        (Some("any"), None, None) => ChildJoin::Any,
        (Some("quorum"), Some(count), None) => ChildJoin::Quorum {
            count: ChildQuorum::new(count.parse::<u16>().map_err(|_| unsupported_guard())?)?,
        },
        _ => return Err(unsupported_guard()),
    };
    Ok(GuardCondition::ChildrenCompleted(
        ChildrenCompletedCondition::new(step_id, join),
    ))
}

fn parse_output_field_guard(check: &str) -> Result<GuardCondition, DomainError> {
    let body = check
        .strip_prefix("output_field:")
        .ok_or_else(unsupported_guard)?;
    let (step, comparison) = body.split_once(':').ok_or_else(unsupported_guard)?;
    let step_id = StepId::new(step)?;
    let mut parsed = comparison.match_indices('=').filter_map(|(index, _)| {
        let field = StepOutputField::new(&comparison[..index]).ok()?;
        let expected = serde_json::from_str(&comparison[index + 1..]).ok()?;
        Some((field, expected))
    });
    let Some((output_field, expected)) = parsed.next() else {
        return Err(unsupported_guard());
    };
    if parsed.next().is_some() {
        return Err(DomainError::InvalidDocument {
            reason: "ambiguous ceremony output-field guard".to_owned(),
        });
    }
    Ok(GuardCondition::OutputField(OutputFieldGuardCondition::new(
        step_id,
        output_field,
        expected,
    )))
}

fn unsupported_guard() -> DomainError {
    DomainError::InvariantViolated {
        reason: "unsupported ceremony guard check",
    }
}

fn parse_step_status_guard(check: &str) -> Result<GuardCondition, DomainError> {
    let parts = check.split(':').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0] != "step_status" {
        return Err(unsupported_guard());
    }
    Ok(GuardCondition::StepStatus {
        step_id: StepId::new(parts[1])?,
        status: StepStatus::try_from(parts[2])?,
    })
}
