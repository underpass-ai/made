use super::{
    CeremonyDefinition, CeremonyInstance, CeremonyTransitionRecord, DomainError, OffsetDateTime,
    RoleAction, RoleId, StateId, TransitionTrigger,
};

impl CeremonyInstance {
    pub fn apply_transition_as(
        &mut self,
        definition: &CeremonyDefinition,
        role_id: &RoleId,
        trigger: &TransitionTrigger,
        now: OffsetDateTime,
    ) -> Result<StateId, DomainError> {
        self.require_role(
            definition,
            role_id,
            &RoleAction::transition(trigger.clone()),
        )?;
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

    /// The one place a session moves.
    ///
    /// `applied_by` is absent when the engine took the move itself,
    /// and naming somebody would be inventing them.
    fn move_on(
        &mut self,
        definition: &CeremonyDefinition,
        trigger: &TransitionTrigger,
        applied_by: Option<RoleId>,
        now: OffsetDateTime,
    ) -> Result<StateId, DomainError> {
        self.require_definition(definition)?;
        if self.is_terminal(definition) {
            return Err(DomainError::InvariantViolated {
                reason: "terminal ceremony instances cannot transition",
            });
        }

        let transition = definition
            .transition_for_trigger(&self.current_state, trigger)
            .ok_or(DomainError::InvalidTransition {
                from: "ceremony_instance.current_state",
                to: "transition_trigger",
            })?;
        if !definition.repeat_requirements_are_satisfied(&self.current_state, &self.step_records) {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony step repeat condition is not satisfied",
            });
        }
        if !definition.guards_are_satisfied(transition, &self.step_records, &self.context) {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony transition guards are not satisfied",
            });
        }

        let from_state = self.current_state.clone();
        self.current_state = transition.to().clone();
        self.transitions.push(CeremonyTransitionRecord::record(
            trigger.clone(),
            from_state,
            self.current_state.clone(),
            applied_by,
            now,
        ));
        self.updated_at = now;
        if definition.is_terminal_state(&self.current_state) {
            self.completed_at = Some(now);
        }
        Ok(self.current_state.clone())
    }
}
