use made_app::usecases::{
    ProtocolDesignJoin, ProtocolDesignPatternStage, ProtocolStagePatternKind,
};
use made_core::error::DomainError;
use made_core::value_objects::{JoinStepCount, RoleId, StateIteration, StepId, StepInstructions};
use serde::Deserialize;

use super::pattern_intent::PatternIntent;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::embedded::embedded_design_protocol_request) struct PatternStageIntent {
    id: String,
    pattern: PatternIntent,
}

impl PatternStageIntent {
    pub(super) fn into_domain(self) -> Result<ProtocolDesignPatternStage, DomainError> {
        let mut stage = ProtocolDesignPatternStage::new(
            StepId::new(self.id)?,
            ProtocolStagePatternKind::parse(&self.pattern.kind)?,
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
                ("all_steps_completed", None) => ProtocolDesignJoin::AllStepsCompleted,
                ("any_step_completed", None) => ProtocolDesignJoin::AnyStepCompleted,
                ("steps_completed", Some(count)) => {
                    ProtocolDesignJoin::StepsCompleted(JoinStepCount::new(count)?)
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
