use crate::entities::ceremony_commands::ApplyTransition;
use crate::entities::CeremonyCommand;

use super::{
    CeremonyDefinition, CeremonyEvent, CeremonyInstance, DomainError, OffsetDateTime, RoleId,
    StateId, TransitionTrigger,
};

impl CeremonyInstance {
    pub fn apply_transition_as(
        &mut self,
        definition: &CeremonyDefinition,
        role_id: &RoleId,
        trigger: &TransitionTrigger,
        now: OffsetDateTime,
    ) -> Result<StateId, DomainError> {
        self.move_on(definition, trigger, Some(role_id.clone()), now)
    }

    pub fn apply_transition(
        &mut self,
        definition: &CeremonyDefinition,
        trigger: &TransitionTrigger,
        now: OffsetDateTime,
    ) -> Result<StateId, DomainError> {
        self.move_on(definition, trigger, None, now)
    }

    /// `applied_by` is absent when the engine took the move itself,
    /// and naming somebody would be inventing them.
    fn move_on(
        &mut self,
        definition: &CeremonyDefinition,
        trigger: &TransitionTrigger,
        applied_by: Option<RoleId>,
        now: OffsetDateTime,
    ) -> Result<StateId, DomainError> {
        let command = CeremonyCommand::ApplyTransition(ApplyTransition {
            role_id: applied_by,
            trigger: trigger.clone(),
            now,
        });
        let events = self.decide(&command, definition)?;
        let to_state = events
            .iter()
            .find_map(|event| match event {
                CeremonyEvent::TransitionApplied(applied) => {
                    Some(applied.transition.to_state().clone())
                }
                _ => None,
            })
            .ok_or(DomainError::InvariantViolated {
                reason: "moving decides a transition",
            })?;
        self.apply_all(&events);
        Ok(to_state)
    }
}
