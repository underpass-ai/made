use super::{
    CeremonyDefinition, CeremonyEvidencePack, CeremonyEvidenceRequest, CeremonyEvidenceSourceId,
    CeremonyGuardApproval, CeremonyGuardDeferral, CeremonyInstance, CeremonyIntervention,
    CeremonyInterventionContent, CeremonyInterventionId, CeremonyInterventionKind,
    CeremonyInterventionProvenance, CeremonyInterventionResponse, CeremonyInterventionTarget,
    CeremonyReason, CeremonyReasonKind, CeremonyRecordRef, CeremonyTransitionRecord, DomainError,
    MemoryConfidence, OffsetDateTime, ReasonAsserter, RoleAction, RoleId,
};

impl CeremonyInstance {
    #[allow(clippy::too_many_arguments)]
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

    #[allow(clippy::too_many_arguments)]
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
        self.require_active(
            definition,
            "terminal ceremony instances cannot accept interventions",
        )?;
        self.require_role(definition, &role_id, &RoleAction::request_intervention())?;
        Self::require_intervention_target(definition, &target)?;
        if let Some(provenance) = provenance.as_ref() {
            self.require_intervention_provenance(definition, &role_id, &target, provenance)?;
        }
        if self
            .interventions
            .iter()
            .any(|intervention| intervention.id() == &intervention_id)
        {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_intervention",
            });
        }
        let intervention = CeremonyIntervention::open_with_provenance(
            intervention_id,
            kind,
            role_id,
            target,
            content,
            provenance,
            now,
        );
        self.interventions.push(intervention);
        self.updated_at = now;
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
        self.require_active(
            definition,
            "terminal ceremony instances cannot receive intervention responses",
        )?;
        self.require_role(definition, &role_id, &RoleAction::respond_to_intervention())?;
        self.interventions
            .iter_mut()
            .find(|intervention| intervention.id() == intervention_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_intervention",
            })?
            .respond(role_id, content, now)?;
        self.record_that_it_answers(intervention_id, now);
        self.updated_at = now;
        Ok(())
    }

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
        self.require_active(
            definition,
            "terminal ceremony instances cannot receive intervention evidence",
        )?;
        self.require_role(definition, &role_id, &RoleAction::respond_to_intervention())?;
        self.interventions
            .iter_mut()
            .find(|intervention| intervention.id() == intervention_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_intervention",
            })?
            .respond_with_evidence(role_id, evidence_pack, now)?;
        self.record_that_it_answers(intervention_id, now);
        self.updated_at = now;
        Ok(())
    }

    /// State why one thing here led to another.
    ///
    /// Its own act rather than a field on contributing, because a
    /// reason is often known later — "in fact I did that because…" is
    /// how people reason — and because a field gets filled in by
    /// inertia while an act is chosen.
    ///
    /// What it refuses is the point:
    ///
    /// - a kind only the engine may assert, because a participant able
    ///   to relabel the structure could rewrite the session's shape;
    /// - a kind only an author may assert, claimed by anyone else,
    ///   because nobody else has access to another's reasoning;
    /// - either end naming something this session never produced.
    #[allow(clippy::too_many_arguments)]
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
        self.require_declared_role(definition, &role_id)?;
        self.require_record(&from)?;
        self.require_record(&to)?;

        match kind.asserter() {
            ReasonAsserter::TheEngine => {
                return Err(DomainError::InvariantViolated {
                    reason:
                        "this kind of reason states the shape of the session, not a judgement, \
                             and only the engine may assert it",
                });
            }
            ReasonAsserter::ItsAuthor => {
                if self.author_of(&from) != Some(&role_id) {
                    return Err(DomainError::InvariantViolated {
                        reason: "only whoever produced something may say why they decided it or \
                                 how they did it",
                    });
                }
            }
            ReasonAsserter::AnySeat => {}
        }

        self.reasons.push(CeremonyReason::new(
            from,
            to,
            kind,
            why,
            confidence,
            Some(role_id),
            now,
        )?);
        self.updated_at = now;
        Ok(())
    }

    /// Who produced a record, where anyone did.
    ///
    /// A step has none: the engine ran it. A transition the engine
    /// took has none either. Both are absences rather than gaps, and
    /// they are what stops a reason of testimony being made about
    /// something nobody can testify to.
    fn author_of(&self, record: &CeremonyRecordRef) -> Option<&RoleId> {
        match record {
            CeremonyRecordRef::Step { .. } => None,
            CeremonyRecordRef::AgendaItem { agenda_item } => self
                .intervention(agenda_item)
                .map(CeremonyIntervention::requested_by),
            CeremonyRecordRef::Contribution {
                agenda_item,
                ordinal,
            } => self
                .intervention(agenda_item)
                .and_then(|item| item.responses().get(*ordinal as usize))
                .map(CeremonyInterventionResponse::role_id),
            CeremonyRecordRef::GuardDecision { guard_name } => self
                .guard_approvals
                .iter()
                .find(|approval| approval.guard_name() == guard_name)
                .map(CeremonyGuardApproval::approved_by)
                .or_else(|| {
                    self.guard_deferrals
                        .iter()
                        .find(|deferral| deferral.guard_name() == guard_name)
                        .map(CeremonyGuardDeferral::deferred_by)
                }),
            CeremonyRecordRef::Transition { ordinal } => self
                .transitions
                .get(ordinal.saturating_sub(1) as usize)
                .and_then(CeremonyTransitionRecord::applied_by),
        }
    }

    /// A record this session actually produced.
    ///
    /// Memory cannot check this — an edge there may reach something
    /// written an hour ago — but a session knows everything it has
    /// done, and letting a reason cite what never happened would be
    /// declining to use the one advantage it has.
    fn require_record(&self, record: &CeremonyRecordRef) -> Result<(), DomainError> {
        let exists = match record {
            CeremonyRecordRef::Step { step_id } => self.step_records.contains_key(step_id),
            CeremonyRecordRef::AgendaItem { agenda_item } => {
                self.intervention(agenda_item).is_some()
            }
            CeremonyRecordRef::Contribution {
                agenda_item,
                ordinal,
            } => self
                .intervention(agenda_item)
                .is_some_and(|item| item.responses().len() > *ordinal as usize),
            CeremonyRecordRef::GuardDecision { guard_name } => {
                self.guard_approvals
                    .iter()
                    .any(|approval| approval.guard_name() == guard_name)
                    || self
                        .guard_deferrals
                        .iter()
                        .any(|deferral| deferral.guard_name() == guard_name)
            }
            CeremonyRecordRef::Transition { ordinal } => {
                *ordinal >= 1 && (*ordinal as usize) <= self.transitions.len()
            }
        };
        if exists {
            Ok(())
        } else {
            Err(DomainError::NotFound {
                what: "ceremony_record",
            })
        }
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

    pub fn close_intervention_as(
        &mut self,
        definition: &CeremonyDefinition,
        intervention_id: &CeremonyInterventionId,
        role_id: &RoleId,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.require_active(
            definition,
            "terminal ceremony instances cannot close interventions",
        )?;
        self.require_role(definition, role_id, &RoleAction::request_intervention())?;
        self.interventions
            .iter_mut()
            .find(|intervention| intervention.id() == intervention_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_intervention",
            })?
            .close(role_id, now)?;
        self.updated_at = now;
        Ok(())
    }
}
