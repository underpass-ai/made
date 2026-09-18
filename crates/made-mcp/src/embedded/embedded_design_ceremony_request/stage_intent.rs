use made_app::usecases::CeremonyDesignStage;
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyStepAggregation, ContextKey, ContextWrites, DynamicRoleBinding, NumAgents,
    PriorContext, RoleId, Rounds, StepHandlerKind, StepId, StepInstructions, StepOutputField,
};
use serde::Deserialize;
use std::collections::BTreeMap;

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
    #[serde(default)]
    role_from: Option<String>,
    #[serde(default)]
    allowed_roles: Vec<String>,
    #[serde(default)]
    context_writes: BTreeMap<String, String>,
    #[serde(default)]
    aggregate: Option<CeremonyStepAggregation>,
}

impl StageIntent {
    pub(super) fn into_domain(self) -> Result<CeremonyDesignStage, DomainError> {
        let exit_guards = self
            .exit_guards
            .into_iter()
            .map(ExitGuardIntent::into_domain)
            .collect::<Result<Vec<_>, _>>()?;
        let mut stage = CeremonyDesignStage::new(
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
        .with_exit_guards(exit_guards);
        if let Some(aggregation) = self.aggregate {
            stage = stage.with_aggregation(aggregation);
        }
        match (self.role_from, self.allowed_roles.is_empty()) {
            (Some(role_from), false) => {
                stage = stage.with_dynamic_role_binding(DynamicRoleBinding::new(
                    ContextKey::from_role_from(&role_from)?,
                    self.allowed_roles
                        .into_iter()
                        .map(RoleId::new)
                        .collect::<Result<Vec<_>, _>>()?,
                )?);
            }
            (None, true) => {}
            _ => {
                return Err(DomainError::InvalidDocument {
                    reason: "role_from and allowed_roles must be declared together".to_owned(),
                });
            }
        }
        let writes = self
            .context_writes
            .into_iter()
            .map(|(destination, source)| {
                Ok((ContextKey::new(destination)?, StepOutputField::new(source)?))
            })
            .collect::<Result<BTreeMap<_, _>, DomainError>>()?;
        Ok(stage.with_context_writes(ContextWrites::new(writes)))
    }
}
