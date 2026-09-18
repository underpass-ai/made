//! Deciding: what a command would do to this session, as events.
//!
//! Every invariant, lease, idempotency, authorization and
//! terminal-state rule the aggregate holds is checked here, against
//! the definition and the current state, and nothing is written. A
//! refused command returns the same error the mutator it replaces
//! returned; an accepted one returns the events whose fold is exactly
//! that mutator's effect. One file per command family.

use crate::entities::{CeremonyCommand, CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;

mod children;
mod guard_decisions;
mod interventions;
mod participant_bindings;
mod reasons;
mod start;
mod step_execution;
mod transitions;

impl CeremonyInstance {
    /// Decide a command against this session and its definition.
    ///
    /// Pure: reads the session, writes nothing, and returns the events
    /// that would change it — usually one; two where one act has two
    /// facts to record, as a transition into a terminal state also
    /// completes the ceremony. Fold them with [`Self::apply`] to get
    /// the state the corresponding mutator would have left.
    pub fn decide(
        &self,
        command: &CeremonyCommand,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        match command {
            CeremonyCommand::BindParticipant(command) => {
                self.decide_bind_participant(command, definition)
            }
            CeremonyCommand::StartStep(command) => self.decide_start_step(command, definition),
            CeremonyCommand::ApplyStepResult(command) => {
                self.decide_apply_step_result(command, definition)
            }
            CeremonyCommand::ApplyTransition(command) => {
                self.decide_apply_transition(command, definition)
            }
            CeremonyCommand::ApproveGuard(command) => {
                self.decide_approve_guard(command, definition)
            }
            CeremonyCommand::DeferGuard(command) => self.decide_defer_guard(command, definition),
            CeremonyCommand::RequestIntervention(command) => {
                self.decide_request_intervention(command, definition)
            }
            CeremonyCommand::RespondToIntervention(command) => {
                self.decide_respond_to_intervention(command, definition)
            }
            CeremonyCommand::RespondToInterventionWithEvidence(command) => {
                self.decide_respond_to_intervention_with_evidence(command, definition)
            }
            CeremonyCommand::AssertReason(command) => {
                self.decide_assert_reason(command, definition)
            }
            CeremonyCommand::CloseIntervention(command) => {
                self.decide_close_intervention(command, definition)
            }
            CeremonyCommand::PlanCeremonyChildren(command) => {
                self.decide_plan_children(command, definition)
            }
            CeremonyCommand::AdoptChildSpawnPlan(command) => {
                self.decide_adopt_child_plan(command, definition)
            }
            CeremonyCommand::AcceptChildCompletion(command) => {
                self.decide_accept_child_completion(command, definition)
            }
        }
    }
}
