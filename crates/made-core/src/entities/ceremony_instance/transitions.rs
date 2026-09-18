use crate::entities::ceremony_commands::ApplyTransition;
use crate::entities::CeremonyCommand;
use crate::value_objects::CeremonyTransition;

use super::{
    CeremonyDefinition, CeremonyEvent, CeremonyInstance, DomainError, OffsetDateTime, RoleId,
    StateId, TransitionTrigger,
};

impl CeremonyInstance {
    /// A terminal move also requires every open intervention to be resolved.
    #[must_use]
    pub fn transition_is_enabled(
        &self,
        definition: &CeremonyDefinition,
        transition: &CeremonyTransition,
    ) -> bool {
        self.transition_budget_allows(definition, transition)
            && self.state_repeat_permits_transition(definition)
            && definition.guards_are_satisfied(transition, &self.step_records, &self.context)
            && self
                .require_interventions_resolved_before_entering(definition, transition.to())
                .is_ok()
    }

    pub(super) fn require_transition_budget(
        &self,
        definition: &CeremonyDefinition,
        transition: &CeremonyTransition,
    ) -> Result<(), DomainError> {
        if definition.max_transitions().is_some_and(|limit| {
            u64::try_from(self.transitions.len()).unwrap_or(u64::MAX) >= u64::from(limit.get())
        }) {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony transition limit exhausted",
            });
        }
        if definition.max_bounces().is_some_and(|limit| {
            u64::try_from(self.exact_edge_count(transition)).unwrap_or(u64::MAX)
                >= u64::from(limit.get())
        }) {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony transition bounce limit exhausted",
            });
        }
        Ok(())
    }

    fn transition_budget_allows(
        &self,
        definition: &CeremonyDefinition,
        transition: &CeremonyTransition,
    ) -> bool {
        self.require_transition_budget(definition, transition)
            .is_ok()
    }

    fn exact_edge_count(&self, transition: &CeremonyTransition) -> usize {
        self.transitions
            .iter()
            .filter(|record| {
                record.from_state() == transition.from()
                    && record.trigger() == transition.trigger()
                    && record.to_state() == transition.to()
            })
            .count()
    }

    #[must_use]
    pub fn transition_is_enabled_at(
        &self,
        definition: &CeremonyDefinition,
        transition: &CeremonyTransition,
        now: OffsetDateTime,
    ) -> bool {
        !self.has_live_step_leases_at(definition, now)
            && self.transition_is_enabled(definition, transition)
    }

    pub(super) fn require_interventions_resolved_before_entering(
        &self,
        definition: &CeremonyDefinition,
        state_id: &StateId,
    ) -> Result<(), DomainError> {
        if definition.is_terminal_state(state_id)
            && self
                .interventions
                .iter()
                .any(|item| item.status().is_open())
        {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony cannot enter a terminal state with open interventions",
            });
        }
        Ok(())
    }

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
