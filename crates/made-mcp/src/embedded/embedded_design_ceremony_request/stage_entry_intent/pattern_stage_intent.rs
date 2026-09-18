use made_app::usecases::{
    CeremonyDesignJoin, CeremonyDesignPatternStage, CeremonyStagePatternKind,
};
use made_core::error::DomainError;
use made_core::value_objects::{JoinStepCount, RoleId, StateIteration, StepId, StepInstructions};
use serde::Deserialize;

use super::join_intent::JoinIntent;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PatternIntent {
    kind: String,
    roles: Vec<String>,
    instructions: String,
    #[serde(default)]
    manager_role_id: Option<String>,
    #[serde(default)]
    max_iterations: Option<u32>,
    #[serde(default)]
    fallback_role_id: Option<String>,
    #[serde(default)]
    join: Option<JoinIntent>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::embedded::embedded_design_ceremony_request) struct PatternStageIntent {
    id: String,
    pattern: PatternIntent,
}

impl PatternStageIntent {
    pub(super) fn into_domain(self) -> Result<CeremonyDesignPatternStage, DomainError> {
        let mut stage = CeremonyDesignPatternStage::new(
            StepId::new(self.id)?,
            CeremonyStagePatternKind::parse(&self.pattern.kind)?,
            self.pattern
                .roles
                .into_iter()
                .map(RoleId::new)
                .collect::<Result<Vec<_>, _>>()?,
            StepInstructions::new(self.pattern.instructions)?,
        );
        if let Some(role) = self.pattern.manager_role_id {
            stage = stage.with_manager_role(RoleId::new(role)?);
        }
        if let Some(cap) = self.pattern.max_iterations {
            stage = stage.with_max_iterations(StateIteration::new(cap)?);
        }
        if let Some(role) = self.pattern.fallback_role_id {
            stage = stage.with_fallback_role(RoleId::new(role)?);
        }
        if let Some(join) = self.pattern.join {
            stage = stage.with_join(match (join.condition.as_str(), join.count) {
                ("all_steps_completed", None) => CeremonyDesignJoin::AllStepsCompleted,
                ("any_step_completed", None) => CeremonyDesignJoin::AnyStepCompleted,
                ("steps_completed", Some(count)) => {
                    CeremonyDesignJoin::StepsCompleted(JoinStepCount::new(count)?)
                }
                _ => {
                    return Err(DomainError::InvalidDocument {
                        reason: "invalid pattern join condition or count".to_owned(),
                    })
                }
            });
        }
        Ok(stage)
    }
}
