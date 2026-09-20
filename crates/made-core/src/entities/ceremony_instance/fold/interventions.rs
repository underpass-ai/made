use time::OffsetDateTime;

use crate::entities::ceremony_events::{
    EvidenceCollected, InterventionClosed, InterventionDeliveryAcknowledged, InterventionRequested,
    InterventionResponded,
};
use crate::entities::{CeremonyInstance, CeremonyIntervention};
use crate::value_objects::{
    CeremonyInterventionId, CeremonyReason, CeremonyReasonKind, CeremonyRecordRef, MemoryConfidence,
};

impl CeremonyInstance {
    pub(super) fn apply_intervention_requested(&mut self, requested: &InterventionRequested) {
        self.updated_at = requested.intervention.created_at();
        self.interventions.push(requested.intervention.clone());
    }

    /// The response is appended to its item exactly as it was
    /// decided, and the one reason the engine sees on its own —
    /// that a contribution answers the item it was made against —
    /// is recorded beside it.
    ///
    /// The item offers no unchecked append, so the response goes in
    /// through the same door a live one does; on a stream the
    /// aggregate produced, the checks behind that door were already
    /// passed when the response was decided.
    pub(super) fn apply_intervention_responded(&mut self, responded: &InterventionResponded) {
        let response = &responded.response;
        let Some(intervention) = self.intervention_mut(&responded.intervention_id) else {
            return;
        };
        // Verbatim, because the event is the answer: rebuilding it from
        // its parts would drop the agent and the offer it names, and a
        // replayed session would then disagree with its own journal
        // about who answered.
        let appended = intervention.accept_response(response.clone());
        if appended.is_ok() {
            self.record_that_it_answers(&responded.intervention_id, response.responded_at());
        }
        self.updated_at = response.responded_at();
    }

    /// A host's statement about one offer, appended to its item.
    ///
    /// Nothing else moves: an acknowledgement is not an answer, and an
    /// item with a dozen of them and no response is still unanswered.
    /// The item's own rule decides whether an acknowledgement is new
    /// or a repeat, so a replayed stream reaches the same list.
    pub(super) fn apply_intervention_delivery_acknowledged(
        &mut self,
        acknowledged: &InterventionDeliveryAcknowledged,
    ) {
        let at = acknowledged.ack.acknowledged_at();
        if let Some(intervention) = self.intervention_mut(&acknowledged.intervention_id) {
            let _appended = intervention.acknowledge_delivery(acknowledged.ack.clone());
        }
        self.updated_at = at;
    }

    pub(super) fn apply_intervention_closed(&mut self, closed: &InterventionClosed) {
        if let Some(intervention) = self.intervention_mut(&closed.intervention_id) {
            // Closed by whoever the event names, at the time it says;
            // the item's own rule decided that already.
            let _closed = intervention.close(&closed.closed_by, closed.closed_at);
        }
        self.updated_at = closed.closed_at;
    }

    /// A receipt that a source was consulted. The answer it produced
    /// travels in the response event sealed with it, which is what
    /// changes the item; this changes nothing but the clock.
    pub(super) fn apply_evidence_collected(&mut self, collected: &EvidenceCollected) {
        self.updated_at = collected.collected_at;
    }

    fn intervention_mut(
        &mut self,
        intervention_id: &CeremonyInterventionId,
    ) -> Option<&mut CeremonyIntervention> {
        self.interventions
            .iter_mut()
            .find(|intervention| intervention.id() == intervention_id)
    }

    /// The reason the engine can see on its own: a contribution is the
    /// reply to the item it was made against.
    ///
    /// The only kind it asserts. Everything explanatory comes from
    /// whoever reasoned, because a session ending well after an action
    /// is not the action having worked.
    fn record_that_it_answers(
        &mut self,
        agenda_item: &CeremonyInterventionId,
        now: OffsetDateTime,
    ) {
        let Some(ordinal) = self
            .intervention(agenda_item)
            .map(|item| item.responses().len())
            .and_then(|count| u32::try_from(count.checked_sub(1)?).ok())
        else {
            return;
        };
        if let Ok(reason) = CeremonyReason::new(
            CeremonyRecordRef::contribution(agenda_item.clone(), ordinal),
            CeremonyRecordRef::agenda_item(agenda_item.clone()),
            CeremonyReasonKind::Answers,
            "a contribution made against this agenda item",
            MemoryConfidence::High,
            None,
            now,
        ) {
            self.reasons.push(reason);
        }
    }
}
