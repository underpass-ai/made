//! Folding: the events of a session, back into its state.
//!
//! [`CeremonyInstance::apply`] writes what one event says and nothing
//! else: no definition, no clock, no validation. Every rule was held
//! when the event was decided, and re-checking here would make a
//! stream that once folded stop folding the day a rule changed. One
//! file per event family; [`CeremonyInstance::rehydrate`] is the
//! whole fold from the opening event on.

use crate::entities::{CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;

mod guard_decisions;
mod interventions;
mod participant_bindings;
mod reasons;
mod start;
mod step_execution;
mod transitions;

impl CeremonyInstance {
    /// Write one event into this session.
    ///
    /// Infallible and total: an event the session cannot honour —
    /// a response to an item it does not hold, a second opening —
    /// leaves it untouched rather than failing, because a fold is
    /// not where a bad stream gets refused. `updated_at` becomes the
    /// event's own timestamp.
    pub fn apply(&mut self, event: &CeremonyEvent) {
        match event {
            // A stream opens once. Applying the opening to a session
            // that already exists is a programming error in the
            // caller, and the session is left exactly as it was: a
            // fold that panicked over it would take a host down over
            // one bad stream, and one that reopened the session would
            // silently discard everything after the first opening.
            CeremonyEvent::CeremonyInstanceStarted(_) => {}
            CeremonyEvent::ParticipantsBound(bound) => self.apply_participants_bound(bound),
            CeremonyEvent::StepStarted(started) => self.apply_step_started(started),
            CeremonyEvent::StepCompleted(completed) => self.apply_step_completed(completed),
            CeremonyEvent::StepFailed(failed) => self.apply_step_failed(failed),
            CeremonyEvent::TransitionApplied(applied) => self.apply_transition_applied(applied),
            CeremonyEvent::InterventionRequested(requested) => {
                self.apply_intervention_requested(requested);
            }
            CeremonyEvent::InterventionResponded(responded) => {
                self.apply_intervention_responded(responded);
            }
            CeremonyEvent::InterventionClosed(closed) => self.apply_intervention_closed(closed),
            CeremonyEvent::EvidenceCollected(collected) => self.apply_evidence_collected(collected),
            CeremonyEvent::ReasonAsserted(asserted) => self.apply_reason_asserted(asserted),
            CeremonyEvent::HumanApprovalRecorded(recorded) => {
                self.apply_human_approval_recorded(recorded);
            }
            CeremonyEvent::HumanDeferralRecorded(recorded) => {
                self.apply_human_deferral_recorded(recorded);
            }
            CeremonyEvent::CeremonyCompleted(completed) => self.apply_ceremony_completed(completed),
        }
    }

    /// Fold a whole stream into the session it describes.
    ///
    /// The first event must be the opening; everything after it is
    /// applied in order. A stream that opens with anything else is
    /// not a ceremony's stream and is refused.
    pub fn rehydrate<'a>(
        events: impl IntoIterator<Item = &'a CeremonyEvent>,
    ) -> Result<Self, DomainError> {
        let mut events = events.into_iter();
        let Some(CeremonyEvent::CeremonyInstanceStarted(started)) = events.next() else {
            return Err(DomainError::InvariantViolated {
                reason: "a ceremony stream opens with its start",
            });
        };
        let mut instance = Self::from_started(started);
        for event in events {
            instance.apply(event);
        }
        Ok(instance)
    }

    /// Fold the events one command decided, in order.
    pub(in crate::entities::ceremony_instance) fn apply_all(&mut self, events: &[CeremonyEvent]) {
        for event in events {
            self.apply(event);
        }
    }
}
