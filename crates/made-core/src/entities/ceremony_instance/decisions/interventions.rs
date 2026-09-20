use crate::entities::ceremony_commands::{
    AcknowledgeInterventionDelivery, CloseIntervention, RequestIntervention,
    RespondToIntervention, RespondToInterventionWithEvidence,
};
use crate::entities::ceremony_events::{
    EvidenceCollected, InterventionClosed, InterventionDeliveryAcknowledged, InterventionRequested,
    InterventionResponded,
};
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance, CeremonyIntervention};
use crate::error::DomainError;
use crate::value_objects::{
    CeremonyInterventionId, CeremonyInterventionResponse, DeliveryRecipient, RoleAction,
};

impl CeremonyInstance {
    pub(super) fn decide_request_intervention(
        &self,
        command: &RequestIntervention,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        command.target.validate()?;
        self.require_active(
            definition,
            "terminal ceremony instances cannot accept interventions",
        )?;
        // Authority to ask is not authority to mutate. A supervisor
        // asks under a seat derived from its own principal, so it can
        // neither borrow a declared role's permissions nor be mistaken
        // for one by a reader of the stream. Every other command still
        // goes through `require_role`, which is the whole of what this
        // route buys: one question, and nothing else.
        match command.supervisor.as_ref() {
            Some(supervisor) => {
                if command.role_id != supervisor.requesting_role()? {
                    return Err(DomainError::InvariantViolated {
                        reason: "supervisor intervention must be asked under its derived role",
                    });
                }
            }
            None => self.require_role(
                definition,
                &command.role_id,
                &RoleAction::request_intervention(),
            )?,
        }
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
        let mut intervention = CeremonyIntervention::open_with_provenance(
            command.intervention_id.clone(),
            command.kind,
            command.role_id.clone(),
            command.target.clone(),
            command.content.clone(),
            command.provenance.clone(),
            command.now,
        );
        if let Some(intent) = command.intent {
            intervention = intervention.with_intent(intent);
        }
        if let Some(delivery) = command.delivery.clone() {
            intervention = intervention.with_delivery(delivery);
        }
        if let Some(supervisor) = command.supervisor.clone() {
            intervention = intervention.with_supervisor(supervisor);
        }
        Ok(vec![CeremonyEvent::InterventionRequested(
            InterventionRequested { intervention },
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
        let intervention = self.require_open_intervention(&command.intervention_id)?;
        intervention.ensure_can_respond(&command.role_id)?;
        let mut response = CeremonyInterventionResponse::new(
            command.role_id.clone(),
            command.content.clone(),
            command.now,
        );
        if let Some(executor) = command.executor.as_ref() {
            Self::require_answering_executor(intervention, &command.role_id, executor)?;
            let delivery_id = command.delivery_id.clone().ok_or({
                DomainError::InvariantViolated {
                    reason: "an answer naming its executor must name the delivery it answers",
                }
            })?;
            response = response.answering_delivery(executor.clone(), delivery_id);
        }
        Ok(vec![CeremonyEvent::InterventionResponded(
            InterventionResponded {
                intervention_id: command.intervention_id.clone(),
                response,
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

    /// Seal what a host said about an offer of an open item.
    ///
    /// The ledger already refused a foreign or expired lease before
    /// this is reached; what the aggregate adds is the rule the ledger
    /// cannot know — that the agent answering is one this item was
    /// actually put to. An exact target is checked down to the process
    /// generation, because crediting a replacement with its
    /// predecessor's acknowledgement is the failure this whole route
    /// exists to prevent.
    pub(super) fn decide_acknowledge_intervention_delivery(
        &self,
        command: &AcknowledgeInterventionDelivery,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot record intervention deliveries",
        )?;
        let intervention = self.require_open_intervention(&command.intervention_id)?;
        let recipient = command.ack.recipient();
        if !intervention.target().admits_recipient(recipient) {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony intervention was not delivered to this recipient",
            });
        }
        if let Some(existing) = intervention.delivery_ack(command.ack.delivery_id()) {
            // A host that lost its answer and asked again is repeating
            // itself, not changing its mind: the first statement stays
            // sealed and nothing is appended.
            return if existing.agrees_with(&command.ack) {
                Ok(Vec::new())
            } else {
                Err(DomainError::Conflict {
                    what: "ceremony_intervention.delivery_acknowledgement",
                })
            };
        }
        Ok(vec![CeremonyEvent::InterventionDeliveryAcknowledged(
            InterventionDeliveryAcknowledged {
                intervention_id: command.intervention_id.clone(),
                ack: command.ack.clone(),
            },
        )])
    }

    fn require_answering_executor(
        intervention: &CeremonyIntervention,
        role_id: &crate::value_objects::RoleId,
        executor: &DeliveryRecipient,
    ) -> Result<(), DomainError> {
        if executor.role_id() != role_id {
            return Err(DomainError::InvariantViolated {
                reason: "answering executor does not hold the responding role",
            });
        }
        if !intervention.target().admits_recipient(executor) {
            return Err(DomainError::InvariantViolated {
                reason: "answering executor is not the one this intervention was put to",
            });
        }
        Ok(())
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
