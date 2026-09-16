use crate::entities::ceremony_commands::ApplyTransition;
use crate::entities::ceremony_events::{CeremonyCompleted, TransitionApplied};
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;
use crate::value_objects::{CeremonyTransitionRecord, RoleAction};

impl CeremonyInstance {
    /// The one place a session moves.
    ///
    /// A move into a terminal state is also the ceremony's completion,
    /// so it yields two events: the move, then the ending. They are
    /// decided together so no caller can seal one without the other.
    pub(super) fn decide_apply_transition(
        &self,
        command: &ApplyTransition,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        if let Some(role_id) = command.role_id.as_ref() {
            self.require_role(
                definition,
                role_id,
                &RoleAction::transition(command.trigger.clone()),
            )?;
        }
        self.require_definition(definition)?;
        if self.is_terminal(definition) {
            return Err(DomainError::InvariantViolated {
                reason: "terminal ceremony instances cannot transition",
            });
        }

        let transition = definition
            .transition_for_trigger(&self.current_state, &command.trigger)
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

        let to_state = transition.to().clone();
        let mut events = vec![CeremonyEvent::TransitionApplied(TransitionApplied {
            transition: CeremonyTransitionRecord::record(
                command.trigger.clone(),
                self.current_state.clone(),
                to_state.clone(),
                command.role_id.clone(),
                command.now,
            ),
        })];
        if definition.is_terminal_state(&to_state) {
            events.push(CeremonyEvent::CeremonyCompleted(CeremonyCompleted {
                final_state: to_state,
                completed_at: command.now,
            }));
        }
        Ok(events)
    }
}
