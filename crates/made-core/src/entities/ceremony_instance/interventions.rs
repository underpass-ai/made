use crate::entities::ceremony_commands::{
    AssertReason, CloseIntervention, RequestIntervention, RespondToIntervention,
    RespondToInterventionWithEvidence,
};
use crate::entities::CeremonyCommand;

use super::{
    CeremonyDefinition, CeremonyEvidencePack, CeremonyEvidenceRequest, CeremonyEvidenceSourceId,
    CeremonyInstance, CeremonyInterventionContent, CeremonyInterventionId,
    CeremonyInterventionKind, CeremonyInterventionProvenance, CeremonyInterventionTarget,
    CeremonyReason, CeremonyReasonKind, CeremonyRecordRef, DomainError, MemoryConfidence,
    OffsetDateTime, RoleAction, RoleId,
};

impl CeremonyInstance {
    pub fn request_intervention_as(
        &mut self,
        definition: &CeremonyDefinition,
        intervention_id: CeremonyInterventionId,
        role_id: RoleId,
        kind: CeremonyInterventionKind,
        target: CeremonyInterventionTarget,
        content: CeremonyInterventionContent,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.request_intervention_with_provenance_as(
            definition,
            intervention_id,
            role_id,
            kind,
            target,
            content,
            None,
            now,
        )
    }

    pub fn request_intervention_with_provenance_as(
        &mut self,
        definition: &CeremonyDefinition,
        intervention_id: CeremonyInterventionId,
        role_id: RoleId,
        kind: CeremonyInterventionKind,
        target: CeremonyInterventionTarget,
        content: CeremonyInterventionContent,
        provenance: Option<CeremonyInterventionProvenance>,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let command = CeremonyCommand::RequestIntervention(RequestIntervention {
            intervention_id,
            role_id,
            kind,
            target,
            content,
            provenance,
            intent: None,
            delivery: None,
            supervisor: None,
            now,
        });
        let events = self.decide(&command, definition)?;
        self.apply_all(&events);
        Ok(())
    }

    pub fn respond_to_intervention_as(
        &mut self,
        definition: &CeremonyDefinition,
        intervention_id: &CeremonyInterventionId,
        role_id: RoleId,
        content: CeremonyInterventionContent,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let command = CeremonyCommand::RespondToIntervention(RespondToIntervention {
            intervention_id: intervention_id.clone(),
            role_id,
            content,
            executor: None,
            delivery_id: None,
            now,
        });
        let events = self.decide(&command, definition)?;
        self.apply_all(&events);
        Ok(())
    }

    /// A query, not a command: what a source should be asked to
    /// answer an item, checked against the same rules a response is.
    /// It changes nothing, so it decides no event.
    pub fn prepare_evidence_request_as(
        &self,
        definition: &CeremonyDefinition,
        intervention_id: CeremonyInterventionId,
        role_id: RoleId,
        source_id: CeremonyEvidenceSourceId,
        query: CeremonyInterventionContent,
    ) -> Result<CeremonyEvidenceRequest, DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot collect intervention evidence",
        )?;
        self.require_role(definition, &role_id, &RoleAction::respond_to_intervention())?;
        self.intervention(&intervention_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_intervention",
            })?
            .ensure_can_respond(&role_id)?;
        Ok(CeremonyEvidenceRequest::new(
            self.id.clone(),
            intervention_id,
            role_id,
            source_id,
            query,
            self.context.clone(),
        ))
    }

    pub fn respond_to_intervention_with_evidence_as(
        &mut self,
        definition: &CeremonyDefinition,
        intervention_id: &CeremonyInterventionId,
        role_id: RoleId,
        evidence_pack: CeremonyEvidencePack,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let command =
            CeremonyCommand::RespondToInterventionWithEvidence(RespondToInterventionWithEvidence {
                intervention_id: intervention_id.clone(),
                role_id,
                evidence_pack,
                now,
            });
        let events = self.decide(&command, definition)?;
        self.apply_all(&events);
        Ok(())
    }

    /// State why one thing here led to another.
    ///
    /// Its own act rather than a field on contributing, because a
    /// reason is often known later — "in fact I did that because…" is
    /// how people reason — and because a field gets filled in by
    /// inertia while an act is chosen. What it refuses is decided in
    /// [`Self::decide`]; the reason itself is built first, so a why
    /// that is empty or an edge from a thing to itself is refused by
    /// the reason before the session is consulted.
    pub fn assert_reason_as(
        &mut self,
        definition: &CeremonyDefinition,
        role_id: RoleId,
        from: CeremonyRecordRef,
        to: CeremonyRecordRef,
        kind: CeremonyReasonKind,
        why: impl Into<String>,
        confidence: MemoryConfidence,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let reason = CeremonyReason::new(from, to, kind, why, confidence, Some(role_id), now)?;
        let command = CeremonyCommand::AssertReason(AssertReason { reason });
        let events = self.decide(&command, definition)?;
        self.apply_all(&events);
        Ok(())
    }

    pub fn close_intervention_as(
        &mut self,
        definition: &CeremonyDefinition,
        intervention_id: &CeremonyInterventionId,
        role_id: &RoleId,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        let command = CeremonyCommand::CloseIntervention(CloseIntervention {
            intervention_id: intervention_id.clone(),
            role_id: role_id.clone(),
            now,
        });
        let events = self.decide(&command, definition)?;
        self.apply_all(&events);
        Ok(())
    }
}
