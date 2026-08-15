use made_core::error::DomainError;
use made_core::value_objects::{CeremonyGuard, GuardCondition, GuardName, StepId, StepStatus};
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
        parse_step_status_guard(&self.check)
    }
}

fn parse_step_status_guard(check: &str) -> Result<GuardCondition, DomainError> {
    let parts = check.split(':').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0] != "step_status" {
        return Err(DomainError::InvariantViolated {
            reason: "unsupported ceremony guard check",
        });
    }
    Ok(GuardCondition::StepStatus {
        step_id: StepId::new(parts[1])?,
        status: StepStatus::try_from(parts[2])?,
    })
}
