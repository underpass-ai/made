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
        self.require_admits_new_work("apply_transition")?;
        if self.is_terminal(definition) || self.is_ended() {
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
        self.require_transition_budget(definition, transition)?;
        if self.has_live_step_leases_at(definition, command.now) {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony cannot transition while a step lease is active",
            });
        }
        if !self.state_repeat_permits_transition(definition) {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony state repeat condition is not satisfied",
            });
        }
        if !self.guards_are_satisfied(definition, transition) {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony transition guards are not satisfied",
            });
        }

        let to_state = transition.to().clone();
        let next_visit = self.current_state_visit.next()?;
        let deadline = definition
            .state_timeout()
            .map(|timeout| {
                super::start::checked_deadline(command.now, timeout.duration(), "state_deadline")
            })
            .transpose()?
            .map(|at| crate::value_objects::StateDeadline::new(to_state.clone(), next_visit, at));
        self.require_interventions_resolved_before_entering(definition, &to_state)?;
        let mut events = vec![CeremonyEvent::TransitionApplied(TransitionApplied {
            destination: Some(crate::entities::ceremony_events::StateVisitEntry {
                state_visit: next_visit,
                step_ids: definition
                    .steps_for_state(&to_state)
                    .map(|step| step.id().clone())
                    .collect(),
                deadline,
            }),
            transition: CeremonyTransitionRecord::record_at(
                command.trigger.clone(),
                self.current_state.clone(),
                self.current_state_iteration,
                to_state.clone(),
                command.role_id.clone(),
                command.now,
            )
            .with_state_visit(self.current_state_visit),
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
