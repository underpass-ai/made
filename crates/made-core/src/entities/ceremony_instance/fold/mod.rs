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
mod import;
mod interventions;
mod participant_bindings;
mod reasons;
mod recollection;
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
            CeremonyEvent::StateIterationStarted(started) => {
                self.apply_state_iteration_started(started)
            }
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
            CeremonyEvent::InstanceImported(imported) => self.apply_instance_imported(imported),
            CeremonyEvent::MemoryRecalled(recalled) => self.apply_memory_recalled(recalled),
        }
    }

    /// Fold a whole stream into the session it describes.
    ///
    /// The first event opens the session: either it was started here,
    /// or it was imported from a store written before ceremonies were
    /// streams (ADR-012). A stream that opens with anything else is not
    /// a ceremony's stream and is refused, and so is one that carries
    /// an import anywhere but at its first position — an import
    /// replaces the whole session, so a second one would silently
    /// discard everything between them.
    pub fn rehydrate<'a>(
        events: impl IntoIterator<Item = &'a CeremonyEvent>,
    ) -> Result<Self, DomainError> {
        let mut events = events.into_iter();
        let mut instance = match events.next() {
            Some(CeremonyEvent::CeremonyInstanceStarted(started)) => Self::from_started(started),
            Some(CeremonyEvent::InstanceImported(imported)) => Self::from_imported(imported),
            _ => {
                return Err(DomainError::InvariantViolated {
                    reason: "a ceremony stream opens with its start or with its import",
                })
            }
        };
        for event in events {
            if matches!(event, CeremonyEvent::InstanceImported(_)) {
                return Err(DomainError::InvariantViolated {
                    reason: "a ceremony stream carries an import only as its first event",
                });
            }
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
