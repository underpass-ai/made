//! Folding: the events of a session, back into its state.
//!
//! [`ProtocolInstance::apply`] writes what one event says and nothing
//! else: no definition, no clock, no validation. Every rule was held
//! when the event was decided, and re-checking here would make a
//! stream that once folded stop folding the day a rule changed. One
//! file per event family; [`ProtocolInstance::rehydrate`] is the
//! whole fold from the opening event on.

use crate::entities::{ProtocolEvent, ProtocolInstance};
use crate::error::DomainError;

mod children;
mod context;
mod execution_receipts;
mod guard_decisions;
mod import;
mod interventions;
mod lifecycle;
mod participant_bindings;
mod reasons;
mod recollection;
mod start;
mod step_execution;
mod succession;
mod transitions;

impl ProtocolInstance {
    /// Write one event into this session.
    ///
    /// Infallible and total: an event the session cannot honour —
    /// a response to an item it does not hold, a second opening —
    /// leaves it untouched rather than failing, because a fold is
    /// not where a bad stream gets refused. `updated_at` becomes the
    /// event's own timestamp.
    pub fn apply(&mut self, event: &ProtocolEvent) {
        match event {
            ProtocolEvent::HostHandoffRecorded(recorded) => {
                self.host_handoffs
                    .insert(recorded.declaration.id.clone(), recorded.clone());
                self.updated_at = recorded.recorded_at;
            }
            // A stream opens once. Applying the opening to a session
            // that already exists is a programming error in the
            // caller, and the session is left exactly as it was: a
            // fold that panicked over it would take a host down over
            // one bad stream, and one that reopened the session would
            // silently discard everything after the first opening.
            ProtocolEvent::ProtocolInstanceStarted(_) => {}
            ProtocolEvent::ParticipantsBound(bound) => self.apply_participants_bound(bound),
            ProtocolEvent::StepStarted(started) => self.apply_step_started(started),
            ProtocolEvent::StepLeaseRenewed(renewed) => {
                if let Some(request) = &renewed.request {
                    self.lease_renewals
                        .insert(request.id.clone(), renewed.clone());
                }
                if let Some(record) = self.step_records.get_mut(&renewed.step_id) {
                    record.renew_lease_until(renewed.expires_at);
                }
                self.updated_at = renewed.renewed_at;
            }
            ProtocolEvent::StepCompleted(completed) => self.apply_step_completed(completed),
            ProtocolEvent::StepFailed(failed) => self.apply_step_failed(failed),
            ProtocolEvent::ContextWritten(written) => self.apply_context_written(written),
            ProtocolEvent::StateIterationStarted(started) => {
                self.apply_state_iteration_started(started);
            }
            ProtocolEvent::TransitionApplied(applied) => self.apply_transition_applied(applied),
            ProtocolEvent::InterventionRequested(requested) => {
                self.apply_intervention_requested(requested);
            }
            ProtocolEvent::InterventionResponded(responded) => {
                self.apply_intervention_responded(responded);
            }
            ProtocolEvent::InterventionClosed(closed) => self.apply_intervention_closed(closed),
            ProtocolEvent::InterventionDeliveryAcknowledged(acknowledged) => {
                self.apply_intervention_delivery_acknowledged(acknowledged);
            }
            ProtocolEvent::EvidenceCollected(collected) => self.apply_evidence_collected(collected),
            ProtocolEvent::ReasonAsserted(asserted) => self.apply_reason_asserted(asserted),
            ProtocolEvent::HumanApprovalRecorded(recorded) => {
                self.apply_human_approval_recorded(recorded);
            }
            ProtocolEvent::HumanDeferralRecorded(recorded) => {
                self.apply_human_deferral_recorded(recorded);
            }
            ProtocolEvent::ProtocolCompleted(completed) => self.apply_protocol_completed(completed),
            ProtocolEvent::InstanceImported(imported) => self.apply_instance_imported(imported),
            ProtocolEvent::MemoryRecalled(recalled) => self.apply_memory_recalled(recalled),
            ProtocolEvent::ChildSpawnPlanned(event) => self.apply_child_spawn_planned(event),
            ProtocolEvent::ChildSpawnPlanAdopted(event) => {
                self.apply_child_spawn_plan_adopted(event);
            }
            ProtocolEvent::ChildCompletionAccepted(event) => {
                self.apply_child_completion_accepted(event);
            }
            ProtocolEvent::ProtocolPaused(event) => self.apply_protocol_paused(event),
            ProtocolEvent::ProtocolResumed(event) => self.apply_protocol_resumed(event),
            ProtocolEvent::ProtocolCancelled(event) => self.apply_protocol_cancelled(event),
            ProtocolEvent::ProtocolDeadlineExceeded(event) => {
                self.apply_protocol_deadline_exceeded(event);
            }
            ProtocolEvent::StateDeadlineExceeded(event) => {
                self.apply_state_deadline_exceeded(event);
            }
            ProtocolEvent::StepDeadlineExceeded(event) => self.apply_step_deadline_exceeded(event),
            ProtocolEvent::LateStepResultObserved(event) => {
                self.apply_late_step_result_observed(event);
            }
            ProtocolEvent::ExecutionReceiptLinked(event) => {
                self.apply_execution_receipt_linked(event);
            }
            ProtocolEvent::SuccessorPlanned(event) => self.apply_successor_planned(event),
            ProtocolEvent::SuccessionCarried(event) => self.apply_succession_carried(event),
        }
    }

    /// Fold a whole stream into the session it describes.
    ///
    /// The first event opens the session: either it was started here,
    /// or it was imported from a store written before protocols were
    /// streams (ADR-012). A stream that opens with anything else is not
    /// a protocol's stream and is refused, and so is one that carries
    /// an import anywhere but at its first position — an import
    /// replaces the whole session, so a second one would silently
    /// discard everything between them.
    pub fn rehydrate<'a>(
        events: impl IntoIterator<Item = &'a ProtocolEvent>,
    ) -> Result<Self, DomainError> {
        let mut events = events.into_iter();
        let mut instance = match events.next() {
            Some(ProtocolEvent::ProtocolInstanceStarted(started)) => Self::from_started(started),
            Some(ProtocolEvent::InstanceImported(imported)) => Self::from_imported(imported),
            _ => {
                return Err(DomainError::InvariantViolated {
                    reason: "a protocol stream opens with its start or with its import",
                })
            }
        };
        for event in events {
            if matches!(event, ProtocolEvent::InstanceImported(_)) {
                return Err(DomainError::InvariantViolated {
                    reason: "a protocol stream carries an import only as its first event",
                });
            }
            instance.apply(event);
        }
        Ok(instance)
    }

    /// Fold the events one command decided, in order.
    pub(in crate::entities::protocol_instance) fn apply_all(&mut self, events: &[ProtocolEvent]) {
        for event in events {
            self.apply(event);
        }
    }
}
