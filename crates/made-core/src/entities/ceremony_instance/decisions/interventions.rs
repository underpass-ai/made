use crate::entities::ceremony_commands::{
    CloseIntervention, RequestIntervention, RespondToIntervention,
    RespondToInterventionWithEvidence,
};
use crate::entities::ceremony_events::{
    EvidenceCollected, InterventionClosed, InterventionRequested, InterventionResponded,
};
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance, CeremonyIntervention};
use crate::error::DomainError;
use crate::value_objects::{CeremonyInterventionId, CeremonyInterventionResponse, RoleAction};

impl CeremonyInstance {
    pub(super) fn decide_request_intervention(
        &self,
        command: &RequestIntervention,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot accept interventions",
        )?;
        self.require_role(
            definition,
            &command.role_id,
            &RoleAction::request_intervention(),
        )?;
        Self::require_intervention_target(definition, &command.target)?;
        if let Some(provenance) = command.provenance.as_ref() {
            self.require_intervention_provenance(
                definition,
                &command.role_id,
                &command.target,
                provenance,
            )?;
        }
        if self.intervention(&command.intervention_id).is_some() {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_intervention",
            });
        }
        Ok(vec![CeremonyEvent::InterventionRequested(
            InterventionRequested {
                intervention: CeremonyIntervention::open_with_provenance(
                    command.intervention_id.clone(),
                    command.kind,
                    command.role_id.clone(),
                    command.target.clone(),
                    command.content.clone(),
                    command.provenance.clone(),
                    command.now,
                ),
            },
        )])
    }

    pub(super) fn decide_respond_to_intervention(
        &self,
        command: &RespondToIntervention,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot receive intervention responses",
        )?;
        self.require_role(
            definition,
            &command.role_id,
            &RoleAction::respond_to_intervention(),
        )?;
        self.require_open_intervention(&command.intervention_id)?
            .ensure_can_respond(&command.role_id)?;
        Ok(vec![CeremonyEvent::InterventionResponded(
            InterventionResponded {
                intervention_id: command.intervention_id.clone(),
                response: CeremonyInterventionResponse::new(
                    command.role_id.clone(),
                    command.content.clone(),
                    command.now,
                ),
            },
        )])
    }

    /// Two events, because two things happened: a source was
    /// consulted, and the item was answered. The receipt for the
    /// source comes first, the answer — sealed exactly as a plain
    /// response is — second, as the journal has always ordered them.
    pub(super) fn decide_respond_to_intervention_with_evidence(
        &self,
        command: &RespondToInterventionWithEvidence,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot receive intervention evidence",
        )?;
        self.require_role(
            definition,
            &command.role_id,
            &RoleAction::respond_to_intervention(),
        )?;
        self.require_open_intervention(&command.intervention_id)?
            .ensure_can_respond(&command.role_id)?;
        let response = CeremonyInterventionResponse::from_evidence(
            command.role_id.clone(),
            command.evidence_pack.clone(),
            command.now,
        )?;
        Ok(vec![
            CeremonyEvent::EvidenceCollected(EvidenceCollected {
                intervention_id: command.intervention_id.clone(),
                source_id: command.evidence_pack.source_id().clone(),
                collected_by: command.role_id.clone(),
                evidence_pack: command.evidence_pack.clone(),
                collected_at: command.now,
            }),
            CeremonyEvent::InterventionResponded(InterventionResponded {
                intervention_id: command.intervention_id.clone(),
                response,
            }),
        ])
    }

    pub(super) fn decide_close_intervention(
        &self,
        command: &CloseIntervention,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot close interventions",
        )?;
        self.require_role(
            definition,
            &command.role_id,
            &RoleAction::request_intervention(),
        )?;
        // The intervention knows who may close it and when; asking a
        // copy keeps the rule in one place and this session untouched.
        self.require_open_intervention(&command.intervention_id)?
            .clone()
            .close(&command.role_id, command.now)?;
        Ok(vec![CeremonyEvent::InterventionClosed(
            InterventionClosed {
                intervention_id: command.intervention_id.clone(),
                closed_by: command.role_id.clone(),
                closed_at: command.now,
            },
        )])
    }

    fn require_open_intervention(
        &self,
        intervention_id: &CeremonyInterventionId,
    ) -> Result<&CeremonyIntervention, DomainError> {
        self.intervention(intervention_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_intervention",
            })
    }
}
