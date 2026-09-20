use time::OffsetDateTime;

use crate::entities::ceremony_commands::PlanSuccessor;
use crate::entities::ceremony_events::SuccessorPlanned;
use crate::entities::{
    CeremonyDefinition, CeremonyEvent, CeremonyInstance, PublishedCeremonyDefinition,
};
use crate::error::DomainError;
use crate::value_objects::{
    BudgetDisposition, CarriedEvidence, CeremonyChangeImpact, CeremonyClaimPhase,
    CeremonyDefinitionDiff, CeremonyLifecyclePhase, CeremonyValidationLocus, StepExecutionRecord,
    StepId, StepStatus, SuccessionPlan, SuccessorCeremonyId,
};

impl CeremonyInstance {
    /// Seal a handoff to a successor.
    ///
    /// Decided against the predecessor, because the predecessor is the
    /// authority on what it did and what it still holds. Everything
    /// the successor needs is settled here — which definition, what it
    /// starts with, what becomes of every claim still outstanding — so
    /// the opening that follows has nothing left to decide, and a
    /// crash between the two is resumable rather than ambiguous.
    pub(super) fn decide_plan_successor(
        &self,
        command: &PlanSuccessor,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        let plan = &command.plan;
        if let Some(existing) = &self.successor_plan {
            return if existing == plan {
                Ok(Vec::new())
            } else {
                Err(DomainError::Conflict {
                    what: "successor_already_planned",
                })
            };
        }
        if self.lifecycle.phase() != CeremonyLifecyclePhase::Paused {
            return Err(DomainError::LifecycleRefused {
                operation: "plan_successor",
                phase: self.lifecycle.phase(),
            });
        }
        if plan.budget() == BudgetDisposition::TransferRemaining {
            return Err(DomainError::InvariantViolated {
                reason: "budget transfer is not supported in this release",
            });
        }
        require_pinned_successor(plan, &command.successor)?;
        self.require_derived_successor_id(plan)?;
        self.require_every_outstanding_claim_is_disposed(plan, command.now)?;
        for evidence in plan.carried() {
            self.require_carried_evidence_is_sound(evidence, command)?;
        }
        Ok(vec![CeremonyEvent::SuccessorPlanned(SuccessorPlanned {
            plan: plan.clone(),
        })])
    }

    /// The successor's id follows from this ceremony and this plan, so
    /// a retried handoff cannot open a second successor.
    fn require_derived_successor_id(&self, plan: &SuccessionPlan) -> Result<(), DomainError> {
        let derived = SuccessorCeremonyId::derive(self.id(), plan.plan_id())?;
        if derived.as_ceremony_id() != plan.successor_id() {
            return Err(DomainError::InvariantViolated {
                reason: "successor id is not the one this plan derives",
            });
        }
        Ok(())
    }

    /// Every claim still outstanding has an answer, given against the
    /// claim the plan actually observed.
    ///
    /// A claim whose fence has moved on is a different claim, and
    /// answering for one while another holds the step is exactly the
    /// silent abandonment this rule exists to prevent.
    fn require_every_outstanding_claim_is_disposed(
        &self,
        plan: &SuccessionPlan,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        for (step_id, record) in &self.step_records {
            if !claim_phase(record, now).is_some_and(CeremonyClaimPhase::is_outstanding) {
                continue;
            }
            let disposition =
                plan.disposition_for(step_id)
                    .ok_or_else(|| DomainError::InvalidDocument {
                        reason: format!(
                            "step `{}` holds an outstanding claim with no disposition",
                            step_id.as_str()
                        ),
                    })?;
            self.require_step_claim_fence(step_id, disposition.claim_fence())?;
        }
        for disposition in plan.dispositions() {
            let outstanding = self
                .step_records
                .get(disposition.step_id())
                .and_then(|record| claim_phase(record, now))
                .is_some_and(CeremonyClaimPhase::is_outstanding);
            if !outstanding {
                return Err(DomainError::InvalidDocument {
                    reason: format!(
                        "step `{}` holds no outstanding claim to dispose of",
                        disposition.step_id().as_str()
                    ),
                });
            }
        }
        Ok(())
    }

    /// Carried evidence points at work this ceremony really completed,
    /// and lands on a step the successor declares and the diff carries.
    fn require_carried_evidence_is_sound(
        &self,
        evidence: &CarriedEvidence,
        command: &PlanSuccessor,
    ) -> Result<(), DomainError> {
        let source = evidence.source();
        if source.ceremony_id() != self.id() {
            return Err(DomainError::InvalidDocument {
                reason: format!(
                    "carried evidence for step `{}` names another ceremony as its source",
                    evidence.successor_step_id().as_str()
                ),
            });
        }
        let record = self
            .step_record(source.step_id())
            .ok_or_else(|| DomainError::InvalidDocument {
                reason: format!(
                    "carried evidence names step `{}`, which this ceremony has no record of",
                    source.step_id().as_str()
                ),
            })?;
        if record.status() != StepStatus::Completed {
            return Err(DomainError::InvalidDocument {
                reason: format!(
                    "carried evidence names step `{}`, which this ceremony did not complete",
                    source.step_id().as_str()
                ),
            });
        }
        if record.state_visit() != source.state_visit() || record.attempt() != source.attempt() {
            return Err(DomainError::InvalidDocument {
                reason: format!(
                    "carried evidence for step `{}` names a visit or attempt this ceremony did not seal",
                    source.step_id().as_str()
                ),
            });
        }
        if record.output() != evidence.output() {
            return Err(DomainError::InvalidDocument {
                reason: format!(
                    "carried evidence for step `{}` differs from the output this ceremony sealed",
                    source.step_id().as_str()
                ),
            });
        }
        require_successor_declares(&command.successor, evidence.successor_step_id())?;
        require_step_carries(&command.diff, evidence.successor_step_id())
    }
}

/// Which phase a step record's claim is in, or nothing where the
/// record holds no claim at all.
///
/// The aggregate knows only what it sealed, so `Retired` — a claim the
/// step has since moved past — never arises here: the current record
/// is the current claim by construction.
fn claim_phase(record: &StepExecutionRecord, now: OffsetDateTime) -> Option<CeremonyClaimPhase> {
    match record.status() {
        StepStatus::InProgress if record.has_live_lease_at(now) => Some(CeremonyClaimPhase::Live),
        StepStatus::InProgress => Some(CeremonyClaimPhase::Expired),
        StepStatus::Completed => Some(CeremonyClaimPhase::Completed),
        StepStatus::Failed => Some(CeremonyClaimPhase::Failed),
        _ => None,
    }
}

/// The plan's pin names the bytes the plan was made against.
fn require_pinned_successor(
    plan: &SuccessionPlan,
    successor: &PublishedCeremonyDefinition,
) -> Result<(), DomainError> {
    let pin = plan.successor_definition();
    if pin.name() != successor.name()
        || pin.version() != successor.version()
        || pin.digest() != successor.digest()
    {
        return Err(DomainError::InvariantViolated {
            reason: "the sealed successor publication is not the one this plan pinned",
        });
    }
    Ok(())
}

/// A successor cannot start holding the answer to a question it never
/// asks.
fn require_successor_declares(
    successor: &PublishedCeremonyDefinition,
    successor_step_id: &StepId,
) -> Result<(), DomainError> {
    if successor.definition().step(successor_step_id).is_none() {
        return Err(DomainError::InvalidDocument {
            reason: format!(
                "the successor's definition declares no step `{}`",
                successor_step_id.as_str()
            ),
        });
    }
    Ok(())
}

/// A step the successor's definition strands cannot carry evidence.
///
/// Stranding is exactly the diff saying "a session counting on this
/// cannot be brought across". Carrying anyway would present work done
/// under one definition as work done under another, which is the thing
/// a succession exists so that nobody has to do.
fn require_step_carries(
    diff: &CeremonyDefinitionDiff,
    successor_step_id: &StepId,
) -> Result<(), DomainError> {
    let locus = CeremonyValidationLocus::step(successor_step_id.clone());
    let strands = diff.changes().iter().any(|change| {
        change.locus() == &locus && change.impact() == CeremonyChangeImpact::Strands
    });
    if strands {
        return Err(DomainError::InvalidDocument {
            reason: format!(
                "step `{}` is stranded by the successor's definition and cannot carry evidence",
                successor_step_id.as_str()
            ),
        });
    }
    Ok(())
}
