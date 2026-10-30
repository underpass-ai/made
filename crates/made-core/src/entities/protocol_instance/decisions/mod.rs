//! Deciding: what a command would do to this session, as events.
//!
//! Every invariant, lease, idempotency, authorization and
//! terminal-state rule the aggregate holds is checked here, against
//! the definition and the current state, and nothing is written. A
//! refused command returns the same error the mutator it replaces
//! returned; an accepted one returns the events whose fold is exactly
//! that mutator's effect. One file per command family.

use crate::entities::{ProtocolCommand, ProtocolDefinition, ProtocolEvent, ProtocolInstance};
use crate::error::DomainError;

mod budget_admission;
mod children;
mod execution_receipts;
mod guard_decisions;
mod host_handoff;
mod interventions;
mod lease_renewal;
mod lifecycle;
mod participant_bindings;
mod reasons;
mod start;
mod step_execution;
mod succession;
mod transitions;

impl ProtocolInstance {
    /// Decide a command against this session and its definition.
    ///
    /// Pure: reads the session, writes nothing, and returns the events
    /// that would change it — usually one; two where one act has two
    /// facts to record, as a transition into a terminal state also
    /// completes the protocol. Fold them with [`Self::apply`] to get
    /// the state the corresponding mutator would have left.
    pub fn decide(
        &self,
        command: &ProtocolCommand,
        definition: &ProtocolDefinition,
    ) -> Result<Vec<ProtocolEvent>, DomainError> {
        match command {
            ProtocolCommand::RecordHostHandoff(command) => {
                self.decide_record_host_handoff(command, definition)
            }
            ProtocolCommand::BindParticipant(command) => {
                self.decide_bind_participant(command, definition)
            }
            ProtocolCommand::StartStep(command) => self.decide_start_step(command, definition),
            ProtocolCommand::RenewStepLease(command) => {
                self.decide_renew_step_lease(command, definition)
            }
            ProtocolCommand::ApplyStepResult(command) => {
                self.decide_apply_step_result(command, definition)
            }
            ProtocolCommand::ApplyExecutionReceiptResult(command) => {
                self.decide_apply_execution_receipt_result(command, definition)
            }
            ProtocolCommand::ApplyTransition(command) => {
                self.decide_apply_transition(command, definition)
            }
            ProtocolCommand::ApproveGuard(command) => {
                self.decide_approve_guard(command, definition)
            }
            ProtocolCommand::DeferGuard(command) => self.decide_defer_guard(command, definition),
            ProtocolCommand::RequestIntervention(command) => {
                self.decide_request_intervention(command, definition)
            }
            ProtocolCommand::RespondToIntervention(command) => {
                self.decide_respond_to_intervention(command, definition)
            }
            ProtocolCommand::RespondToInterventionWithEvidence(command) => {
                self.decide_respond_to_intervention_with_evidence(command, definition)
            }
            ProtocolCommand::AcknowledgeInterventionDelivery(command) => {
                self.decide_acknowledge_intervention_delivery(command, definition)
            }
            ProtocolCommand::AssertReason(command) => {
                self.decide_assert_reason(command, definition)
            }
            ProtocolCommand::CloseIntervention(command) => {
                self.decide_close_intervention(command, definition)
            }
            ProtocolCommand::PlanProtocolChildren(command) => {
                self.decide_plan_children(command, definition)
            }
            ProtocolCommand::PlanSuccessor(command) => {
                self.decide_plan_successor(command, definition)
            }
            ProtocolCommand::AdoptChildSpawnPlan(command) => {
                self.decide_adopt_child_plan(command, definition)
            }
            ProtocolCommand::AcceptChildCompletion(command) => {
                self.decide_accept_child_completion(command, definition)
            }
            ProtocolCommand::PauseProtocol(command) => self.decide_pause(command, definition),
            ProtocolCommand::ResumeProtocol(command) => self.decide_resume(command, definition),
            ProtocolCommand::CancelProtocol(command) => self.decide_cancel(command, definition),
            ProtocolCommand::EnforceProtocolDeadlines(command) => {
                self.decide_enforce_deadlines(command, definition)
            }
        }
    }
}
