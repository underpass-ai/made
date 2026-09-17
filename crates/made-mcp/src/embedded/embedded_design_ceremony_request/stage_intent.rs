use made_app::usecases::CeremonyDesignStage;
use made_core::error::DomainError;
use made_core::value_objects::{
    NumAgents, PriorContext, RoleId, Rounds, StepHandlerKind, StepId, StepInstructions,
};
use serde::Deserialize;

use super::{ExitGuardIntent, RepeatIntent};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StageIntent {
    id: String,
    owner_role_id: String,
    instructions: String,
    #[serde(default)]
    handler: Option<String>,
    #[serde(default)]
    see_prior: Option<bool>,
    #[serde(default)]
    num_agents: Option<u64>,
    #[serde(default)]
    review_rounds: u64,
    #[serde(default)]
    repeat: Option<RepeatIntent>,
    #[serde(default)]
    exit_guards: Vec<ExitGuardIntent>,
}

impl StageIntent {
    pub(super) fn into_domain(self) -> Result<CeremonyDesignStage, DomainError> {
        let exit_guards = self
            .exit_guards
            .into_iter()
            .map(ExitGuardIntent::into_domain)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CeremonyDesignStage::new(
            StepId::new(self.id)?,
            RoleId::new(self.owner_role_id)?,
            StepInstructions::new(self.instructions)?,
            self.handler.map(StepHandlerKind::new).transpose()?,
            self.see_prior.map(PriorContext::from_visible),
            self.num_agents
                .map(|value| NumAgents::new(u32::try_from(value).unwrap_or(u32::MAX)))
                .transpose()?,
            Rounds::new(u32::try_from(self.review_rounds).unwrap_or(u32::MAX))?,
            self.repeat.map(RepeatIntent::into_domain).transpose()?,
        )
        .with_exit_guards(exit_guards))
    }
}
