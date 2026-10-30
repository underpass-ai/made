//! [`PlanProtocolSuccessorUseCase`] — what a handoff would look like,
//! before anything is sealed.
//!
//! Read-only. It answers the question a person actually has when a
//! definition turns out to be wrong: what changed, what is still
//! outstanding, what could be carried across, and what is in the way.
//! Every one of those is knowable before deciding, and a capability
//! that made the caller find out by trying would be asking them to
//! seal a fact in order to read one.

use std::sync::Arc;

use made_core::entities::{ProtocolInstance, PublishedProtocolDefinition};
use made_core::error::DomainError;
use made_core::ports::ProtocolDefinitionPublicationPort;
use made_core::value_objects::{
    DefinitionPin, ExecutionRecoveryPageLimit, ProtocolChangeImpact, ProtocolDefinitionChange,
    ProtocolDefinitionDiff, ProtocolValidationLocus, StepId, StepStatus,
};

use super::{
    PlanProtocolSuccessorInput, ProposedCarriedStep, ProtocolSuccessionBlocker,
    ProtocolSuccessorPlanView, RequiredClaimDisposition, ResolveProtocolDefinitionUseCase,
};
use crate::services::{succession_evidence, SessionStream};
use crate::workers::{
    InspectProtocolResumeInput, InspectProtocolResumeUseCase, ProtocolResumePreflight,
};

pub struct PlanProtocolSuccessorUseCase {
    definitions: Arc<ResolveProtocolDefinitionUseCase>,
    publications: Arc<dyn ProtocolDefinitionPublicationPort>,
    stream: Arc<SessionStream>,
    preflight: Arc<InspectProtocolResumeUseCase>,
}

impl std::fmt::Debug for PlanProtocolSuccessorUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlanProtocolSuccessorUseCase")
            .finish_non_exhaustive()
    }
}

impl PlanProtocolSuccessorUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveProtocolDefinitionUseCase>,
        publications: Arc<dyn ProtocolDefinitionPublicationPort>,
        stream: Arc<SessionStream>,
        preflight: Arc<InspectProtocolResumeUseCase>,
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
        input: PlanProtocolSuccessorInput,
    ) -> Result<ProtocolSuccessorPlanView, DomainError> {
        let records = self.stream.records(&input.instance_id).await?;
        let session = SessionStream::fold_records(&records)?;
        let (definition, predecessor_definition) =
            self.definitions.execute_pinned(&session.instance).await?;
        let successor = self.publication(&input).await?;
        let diff = ProtocolDefinitionDiff::between(&definition, successor.definition());
        let preflight = self.whole_preflight(&input).await?;

        let proposed_carried = proposed_carried(&session.instance, &records, &successor, &diff)?;
        let required_dispositions = required_dispositions(&preflight);
        let strands = strands_over_completed_work(&session.instance, &diff);
        let blockers = blockers(&session.instance, &strands);

        Ok(ProtocolSuccessorPlanView {
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
        input: &PlanProtocolSuccessorInput,
    ) -> Result<PublishedProtocolDefinition, DomainError> {
        self.publications
            .published(&input.definition_name, &input.definition_version)
            .await?
            .ok_or(DomainError::NotFound {
                what: "published_protocol_definition",
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
        input: &PlanProtocolSuccessorInput,
    ) -> Result<ProtocolResumePreflight, DomainError> {
        let mut report = self
            .preflight
            .execute(InspectProtocolResumeInput {
                protocol_id: input.instance_id.clone(),
                after_claim: None,
                limit: ExecutionRecoveryPageLimit::DEFAULT,
            })
            .await?;
        while let Some(after_claim) = report.next_after_claim.clone() {
            let next = self
                .preflight
                .execute(InspectProtocolResumeInput {
                    protocol_id: input.instance_id.clone(),
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
    instance: &ProtocolInstance,
    records: &[made_core::entities::AuditRecord],
    successor: &PublishedProtocolDefinition,
    diff: &ProtocolDefinitionDiff,
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

fn required_dispositions(preflight: &ProtocolResumePreflight) -> Vec<RequiredClaimDisposition> {
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

/// Strands that cost this protocol something.
///
/// A change that strands a step nobody ran is not a reason to refuse a
/// handoff; it is the ordinary consequence of changing a definition.
/// Only work already done is at stake.
fn strands_over_completed_work(
    instance: &ProtocolInstance,
    diff: &ProtocolDefinitionDiff,
) -> Vec<ProtocolDefinitionChange> {
    instance
        .step_records()
        .iter()
        .filter(|(_, record)| record.status() == StepStatus::Completed)
        .flat_map(|(step_id, _)| {
            let locus = ProtocolValidationLocus::step(step_id.clone());
            diff.changes()
                .iter()
                .filter(move |change| {
                    change.locus() == &locus && change.impact() == ProtocolChangeImpact::Strands
                })
                .cloned()
        })
        .collect()
}

fn strands(diff: &ProtocolDefinitionDiff, step_id: &StepId) -> bool {
    let locus = ProtocolValidationLocus::step(step_id.clone());
    diff.changes()
        .iter()
        .any(|change| change.locus() == &locus && change.impact() == ProtocolChangeImpact::Strands)
}

fn blockers(
    instance: &ProtocolInstance,
    strands: &[ProtocolDefinitionChange],
) -> Vec<ProtocolSuccessionBlocker> {
    let mut blockers = Vec::new();
    if !instance.is_paused() {
        blockers.push(ProtocolSuccessionBlocker::new(
            "a protocol hands off from a pause; this one is not paused",
        ));
    }
    if instance.is_superseded() {
        blockers.push(ProtocolSuccessionBlocker::new(
            "this protocol has already sealed a handoff to a successor",
        ));
    }
    for change in strands {
        blockers.push(ProtocolSuccessionBlocker::new(format!(
            "{} is stranded by the successor's definition and has already been completed here: {}",
            change.locus(),
            change.detail()
        )));
    }
    blockers
}
