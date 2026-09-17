use made_app::usecases::CeremonyDesignFinalApproval;
use made_core::error::DomainError;
use made_core::value_objects::{GuardName, RoleId, TransitionTrigger};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FinalApprovalIntent {
    role_id: String,
    #[serde(default)]
    guard_name: Option<String>,
    #[serde(default)]
    trigger: Option<String>,
}

impl FinalApprovalIntent {
    pub(super) fn into_domain(self) -> Result<CeremonyDesignFinalApproval, DomainError> {
        Ok(CeremonyDesignFinalApproval::new(
            RoleId::new(self.role_id)?,
            self.guard_name.map(GuardName::new).transpose()?,
            self.trigger.map(TransitionTrigger::new).transpose()?,
        ))
    }
}
