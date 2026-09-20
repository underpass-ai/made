//! [`PlanCeremonySuccessorUseCase`] — what a handoff would look like,
//! before anything is sealed.
//!
//! Read-only. It answers the question a person actually has when a
//! definition turns out to be wrong: what changed, what is still
//! outstanding, what could be carried across, and what is in the way.
//! Every one of those is knowable before deciding, and a capability
//! that made the caller find out by trying would be asking them to
//! seal a fact in order to read one.

use std::sync::Arc;

use made_core::entities::{CeremonyInstance, PublishedCeremonyDefinition};
use made_core::error::DomainError;
use made_core::ports::CeremonyDefinitionPublicationPort;
use made_core::value_objects::{
    CeremonyChangeImpact, CeremonyDefinitionChange, CeremonyDefinitionDiff,
    CeremonyValidationLocus, DefinitionPin, ExecutionRecoveryPageLimit, StepId, StepStatus,
};

use super::{
    CeremonySuccessionBlocker, CeremonySuccessorPlanView, PlanCeremonySuccessorInput,
    ProposedCarriedStep, RequiredClaimDisposition, ResolveCeremonyDefinitionUseCase,
};
use crate::services::{succession_evidence, SessionStream};
use crate::workers::{
    CeremonyResumePreflight, InspectCeremonyResumeInput, InspectCeremonyResumeUseCase,
};

pub struct PlanCeremonySuccessorUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    publications: Arc<dyn CeremonyDefinitionPublicationPort>,
    stream: Arc<SessionStream>,
    preflight: Arc<InspectCeremonyResumeUseCase>,
}

impl std::fmt::Debug for PlanCeremonySuccessorUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlanCeremonySuccessorUseCase")
            .finish_non_exhaustive()
    }
}

impl PlanCeremonySuccessorUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        publications: Arc<dyn CeremonyDefinitionPublicationPort>,
        stream: Arc<SessionStream>,
        preflight: Arc<InspectCeremonyResumeUseCase>,
    ) -> Self {
        Self {
            definitions,
            publications,
            stream,
            preflight,
        }
    }

    pub async fn execute(
        &self,
        input: PlanCeremonySuccessorInput,
    ) -> Result<CeremonySuccessorPlanView, DomainError> {
        let records = self.stream.records(&input.instance_id).await?;
        let session = SessionStream::fold_records(&records)?;
        let definition = self.definitions.execute(&session.instance).await?;
        let predecessor_definition = DefinitionPin::new(
            definition.name().clone(),
            definition.version().clone(),
            definition.digest()?,
        );
        let successor = self.publication(&input).await?;
        let diff = CeremonyDefinitionDiff::between(&definition, successor.definition());
        let preflight = self.whole_preflight(&input).await?;

        let proposed_carried = proposed_carried(&session.instance, &records, &successor, &diff)?;
        let required_dispositions = required_dispositions(&preflight);
        let strands = strands_over_completed_work(&session.instance, &diff);
        let blockers = blockers(&session.instance, &strands);

        Ok(CeremonySuccessorPlanView {
            predecessor_definition,
            successor_definition: DefinitionPin::new(
                successor.name().clone(),
                successor.version().clone(),
                successor.digest(),
            ),
            diff,
            preflight,
            proposed_carried,
            required_dispositions,
            strands,
            ready: blockers.is_empty(),
            blockers,
        })
    }

    async fn publication(
        &self,
        input: &PlanCeremonySuccessorInput,
    ) -> Result<PublishedCeremonyDefinition, DomainError> {
        self.publications
            .published(&input.definition_name, &input.definition_version)
            .await?
            .ok_or(DomainError::NotFound {
                what: "published_ceremony_definition",
            })
    }

    /// The whole preflight, not one page of it.
    ///
    /// A handoff has to answer for every outstanding claim, so a report
    /// that stopped at a page boundary would be a plan that silently
    /// abandoned whatever came after it. The pagination stays where it
    /// is — it is the right shape for an operator reading a long
    /// recovery — and this follows it to the end.
    async fn whole_preflight(
        &self,
        input: &PlanCeremonySuccessorInput,
    ) -> Result<CeremonyResumePreflight, DomainError> {
        let mut report = self
            .preflight
            .execute(InspectCeremonyResumeInput {
                ceremony_id: input.instance_id.clone(),
                after_claim: None,
                limit: ExecutionRecoveryPageLimit::DEFAULT,
            })
            .await?;
        while let Some(after_claim) = report.next_after_claim.clone() {
            let next = self
                .preflight
                .execute(InspectCeremonyResumeInput {
                    ceremony_id: input.instance_id.clone(),
                    after_claim: Some(after_claim),
                    limit: ExecutionRecoveryPageLimit::DEFAULT,
                })
                .await?;
            report.claims.extend(next.claims);
            report.next_after_claim = next.next_after_claim;
        }
        Ok(report)
    }
}

/// Every completed step the successor also declares and the diff does
/// not strand, with the record it would be carried from.
fn proposed_carried(
    instance: &CeremonyInstance,
    records: &[made_core::entities::AuditRecord],
    successor: &PublishedCeremonyDefinition,
    diff: &CeremonyDefinitionDiff,
) -> Result<Vec<ProposedCarriedStep>, DomainError> {
    let mut proposed = Vec::new();
    for (step_id, record) in instance.step_records() {
        if record.status() != StepStatus::Completed
            || successor.definition().step(step_id).is_none()
            || strands(diff, step_id)
        {
            continue;
        }
        if let Some(source) = succession_evidence::completed_source(instance, records, step_id)? {
            proposed.push(ProposedCarriedStep {
                step_id: step_id.clone(),
                source,
            });
        }
    }
    Ok(proposed)
}

fn required_dispositions(preflight: &CeremonyResumePreflight) -> Vec<RequiredClaimDisposition> {
    preflight
        .claims
        .iter()
        .filter(|claim| claim.phase.is_outstanding())
        .map(|claim| RequiredClaimDisposition {
            step_id: claim.step_id.clone(),
            claim_fence: claim.claim_fence.clone(),
            phase: claim.phase,
        })
        .collect()
}

/// Strands that cost this ceremony something.
///
/// A change that strands a step nobody ran is not a reason to refuse a
/// handoff; it is the ordinary consequence of changing a definition.
/// Only work already done is at stake.
fn strands_over_completed_work(
    instance: &CeremonyInstance,
    diff: &CeremonyDefinitionDiff,
) -> Vec<CeremonyDefinitionChange> {
    instance
        .step_records()
        .iter()
        .filter(|(_, record)| record.status() == StepStatus::Completed)
        .flat_map(|(step_id, _)| {
            let locus = CeremonyValidationLocus::step(step_id.clone());
            diff.changes()
                .iter()
                .filter(move |change| {
                    change.locus() == &locus && change.impact() == CeremonyChangeImpact::Strands
                })
                .cloned()
        })
        .collect()
}

fn strands(diff: &CeremonyDefinitionDiff, step_id: &StepId) -> bool {
    let locus = CeremonyValidationLocus::step(step_id.clone());
    diff.changes()
        .iter()
        .any(|change| change.locus() == &locus && change.impact() == CeremonyChangeImpact::Strands)
}

fn blockers(
    instance: &CeremonyInstance,
    strands: &[CeremonyDefinitionChange],
) -> Vec<CeremonySuccessionBlocker> {
    let mut blockers = Vec::new();
    if !instance.is_paused() {
        blockers.push(CeremonySuccessionBlocker::new(
            "a ceremony hands off from a pause; this one is not paused",
        ));
    }
    if instance.is_superseded() {
        blockers.push(CeremonySuccessionBlocker::new(
            "this ceremony has already sealed a handoff to a successor",
        ));
    }
    for change in strands {
        blockers.push(CeremonySuccessionBlocker::new(format!(
            "{} is stranded by the successor's definition and has already been completed here: {}",
            change.locus(),
            change.detail()
        )));
    }
    blockers
}
