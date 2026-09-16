use crate::entities::ceremony_commands::AssertReason;
use crate::entities::ceremony_events::ReasonAsserted;
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance, CeremonyIntervention};
use crate::error::DomainError;
use crate::value_objects::{
    CeremonyGuardApproval, CeremonyGuardDeferral, CeremonyInterventionResponse, CeremonyRecordRef,
    CeremonyTransitionRecord, ReasonAsserter, RoleId,
};

impl CeremonyInstance {
    /// What a reason is refused for is the point:
    ///
    /// - no seat asserting it, because the engine's own reasons are
    ///   recorded by the fold and never decided;
    /// - a kind only the engine may assert, because a participant able
    ///   to relabel the structure could rewrite the session's shape;
    /// - a kind only an author may assert, claimed by anyone else,
    ///   because nobody else has access to another's reasoning;
    /// - either end naming something this session never produced.
    pub(super) fn decide_assert_reason(
        &self,
        command: &AssertReason,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        let reason = &command.reason;
        let role_id = reason.asserted_by().ok_or(DomainError::InvariantViolated {
            reason: "a reason is asserted by a seat; the engine records its own unasked",
        })?;
        self.require_declared_role(definition, role_id)?;
        self.require_record(reason.from())?;
        self.require_record(reason.to())?;

        match reason.kind().asserter() {
            ReasonAsserter::TheEngine => {
                return Err(DomainError::InvariantViolated {
                    reason:
                        "this kind of reason states the shape of the session, not a judgement, \
                             and only the engine may assert it",
                });
            }
            ReasonAsserter::ItsAuthor => {
                if self.author_of(reason.from()) != Some(role_id) {
                    return Err(DomainError::InvariantViolated {
                        reason: "only whoever produced something may say why they decided it or \
                                 how they did it",
                    });
                }
            }
            ReasonAsserter::AnySeat => {}
        }

        Ok(vec![CeremonyEvent::ReasonAsserted(ReasonAsserted {
            reason: reason.clone(),
        })])
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
}
