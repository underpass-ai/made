use super::{
    CeremonyClaimPreflight, CeremonyPreflightAction as Action, CeremonyResumePreflight,
    ClaimExecutionEvidence, InspectCeremonyResumeInput,
};
use crate::services::SessionStream;
use made_core::entities::ceremony_events::StepStarted;
use made_core::entities::{AuditRecord, CeremonyEvent, CeremonyInstance};
use made_core::ports::{ClockPort, ExecutionReceiptStorePort};
use made_core::value_objects::{
    CeremonyDeadline, ExecutionIntent, ExecutionOperationId, ExecutionReceipt, HostWorkState,
    StateDeadline, StepClaimFence, StepDeadline, StepExecutionRecord, StepId, StepStatus,
};
use made_core::DomainError;
use std::sync::Arc;

mod claim_projection;
use claim_projection::{
    claim_phase, effective_expiry, is_in_flight_claim, latest_handoff, permitted_paths,
};

fn imported_claim_phase(
    instance: &CeremonyInstance,
    records: &[AuditRecord],
    step_id: &StepId,
    imported: &StepExecutionRecord,
    fence: &StepClaimFence,
    expiry: time::OffsetDateTime,
    now: time::OffsetDateTime,
) -> super::CeremonyClaimPhase {
    for audit in records.iter().rev() {
        match audit.event() {
            Some(CeremonyEvent::StepCompleted(done))
                if done.step_id == *step_id
                    && done.state_visit() == imported.state_visit()
                    && done.state_iteration() == imported.state_iteration()
                    && done.iteration == imported.iteration()
                    && done.attempt == imported.attempt() =>
            {
                return super::CeremonyClaimPhase::Completed
            }
            Some(CeremonyEvent::StepFailed(done))
                if done.step_id == *step_id
                    && done.state_visit() == imported.state_visit()
                    && done.state_iteration() == imported.state_iteration()
                    && done.iteration == imported.iteration()
                    && done.attempt == imported.attempt() =>
            {
                return super::CeremonyClaimPhase::Failed
            }
            _ => {}
        }
    }
    if !instance.is_ended()
        && instance.step_record(step_id).is_some_and(|current| {
            current.status() == StepStatus::InProgress
                && instance
                    .step_claim_fence(step_id)
                    .is_ok_and(|current_fence| current_fence == *fence)
        })
    {
        if expiry > now {
            super::CeremonyClaimPhase::Live
        } else {
            super::CeremonyClaimPhase::Expired
        }
    } else {
        super::CeremonyClaimPhase::Retired
    }
}

pub struct InspectCeremonyResumeUseCase {
    stream: Arc<SessionStream>,
    receipts: Arc<dyn ExecutionReceiptStorePort>,
    clock: Arc<dyn ClockPort>,
}

mod imported_claim;
use imported_claim::ImportedClaim;

impl std::fmt::Debug for InspectCeremonyResumeUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InspectCeremonyResumeUseCase")
            .finish_non_exhaustive()
    }
}

impl InspectCeremonyResumeUseCase {
    #[must_use]
    pub fn new(
        stream: Arc<SessionStream>,
        receipts: Arc<dyn ExecutionReceiptStorePort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            stream,
            receipts,
            clock,
        }
    }

    #[allow(clippy::too_many_lines)] // one journal-cut projection keeps pagination and readiness coherent
    pub async fn execute(
        &self,
        input: InspectCeremonyResumeInput,
    ) -> Result<CeremonyResumePreflight, DomainError> {
        // One journal cut for engine state and host declarations. Receipt-store reads
        // are later observations and never make this report an execution permit.
        let records = self.stream.records(&input.ceremony_id).await?;
        let session = SessionStream::fold_records(&records)?;
        let instance = &session.instance;
        let now = self.clock.now();
        let ceremony_deadline_at = instance.ceremony_deadline().map(CeremonyDeadline::at);
        let state_deadline_at = instance.state_deadline().map(StateDeadline::at);
        // Ceremony and state deadlines apply to the whole session. Step deadlines
        // are claim-local and are projected below only while their exact claim is
        // still active; a sibling's deadline must not poison another claim.
        let absolute_deadline_overdue = ceremony_deadline_at
            .into_iter()
            .chain(state_deadline_at)
            .any(|at| at <= now);
        let starts: Vec<_> = records
            .iter()
            .filter_map(|record| match record.event() {
                Some(CeremonyEvent::StepStarted(started)) => Some(started),
                _ => None,
            })
            .collect();
        // Imported legacy snapshots can hold an active lease without the
        // historical StepStarted event that predated streams. Project that
        // sealed snapshot claim with its derived fence, rather than silently
        // pretending the host has no work to account for.
        let imported_steps: Vec<ImportedClaim> = records
            .iter()
            .filter_map(|record| match record.event() {
                Some(CeremonyEvent::InstanceImported(imported)) => Some(imported),
                _ => None,
            })
            .flat_map(|imported| {
                imported
                    .snapshot
                    .step_records()
                    .iter()
                    .filter_map(move |(step_id, record)| {
                        (record.status() == StepStatus::InProgress && record.lease().is_some())
                            .then(|| {
                                imported
                                    .snapshot
                                    .step_claim_fence(step_id)
                                    .ok()
                                    .map(|fence| ImportedClaim {
                                        step_id: step_id.clone(),
                                        record: record.clone(),
                                        deadline: imported
                                            .snapshot
                                            .step_deadlines()
                                            .get(step_id)
                                            .cloned(),
                                        fence,
                                    })
                            })
                            .flatten()
                    })
            })
            .filter(|claim| {
                !starts.iter().any(|started| {
                    started
                        .claim_fence(&input.ceremony_id)
                        .is_ok_and(|fence| fence == claim.fence)
                })
            })
            .collect();
        // Compute the session admission barrier over the whole journal cut,
        // before applying the response cursor. A page that happens not to
        // contain an expired sibling must never admit a coordinated resume.
        // Individual claim projections below still receive only absolute
        // deadlines, so this sibling-wide barrier does not mislabel them.
        let any_active_deadline_overdue = starts.iter().any(|started| {
            let fence = started.claim_fence(&input.ceremony_id);
            fence.is_ok_and(|fence| {
                is_in_flight_claim(instance, started, &fence)
                    && started
                        .deadline
                        .as_ref()
                        .is_some_and(|deadline| deadline.at() <= now)
            })
        }) || imported_steps.iter().any(|claim| {
            instance.step_record(&claim.step_id).is_some_and(|current| {
                current.status() == StepStatus::InProgress
                    && instance
                        .step_claim_fence(&claim.step_id)
                        .is_ok_and(|fence| fence == claim.fence)
                    && claim
                        .deadline
                        .as_ref()
                        .is_some_and(|deadline| deadline.at() <= now)
            })
        });
        let deadline_overdue = absolute_deadline_overdue || any_active_deadline_overdue;
        // Only a claim that remains the engine's current InProgress claim can
        // still describe host work. Historical, completed and failed starts
        // are retained for pagination, but cannot require a later host ack.
        let mut has_in_flight_claim = false;
        let mut all_claims_host_reported_quiesced = true;
        for started in &starts {
            let fence = started.claim_fence(&input.ceremony_id)?;
            if is_in_flight_claim(instance, started, &fence) {
                has_in_flight_claim = true;
                all_claims_host_reported_quiesced &= latest_handoff(&records, &fence)
                    .is_some_and(|handoff| handoff.declaration.state == HostWorkState::Quiesced);
            }
        }
        for claim in &imported_steps {
            if !instance.step_record(&claim.step_id).is_some_and(|current| {
                current.status() == StepStatus::InProgress
                    && instance
                        .step_claim_fence(&claim.step_id)
                        .is_ok_and(|fence| fence == claim.fence)
            }) {
                continue;
            }
            has_in_flight_claim = true;
            all_claims_host_reported_quiesced &= latest_handoff(&records, &claim.fence)
                .is_some_and(|handoff| handoff.declaration.state == HostWorkState::Quiesced);
        }
        all_claims_host_reported_quiesced &= has_in_flight_claim;
        let claim_fences = starts
            .iter()
            .map(|started| started.claim_fence(&input.ceremony_id))
            .chain(imported_steps.iter().map(|claim| Ok(claim.fence.clone())))
            .collect::<Result<Vec<_>, _>>()?;
        let offset = match input.after_claim {
            Some(ref after) => {
                claim_fences.iter().position(|fence| fence == after).ok_or(
                    DomainError::NotFound {
                        what: "preflight_claim_cursor",
                    },
                )? + 1
            }
            None => 0,
        };
        let end = (offset + usize::from(input.limit.get())).min(claim_fences.len());
        let mut claims = Vec::with_capacity(end.saturating_sub(offset));
        for index in offset..end {
            if let Some(started) = starts.get(index) {
                claims.push(
                    self.claim(instance, &records, started, now, absolute_deadline_overdue)
                        .await?,
                );
            } else {
                let claim = &imported_steps[index - starts.len()];
                claims.push(
                    self.imported_claim(instance, &records, claim, now, absolute_deadline_overdue)
                        .await?,
                );
            }
        }
        let next_after_claim = (end < claim_fences.len())
            .then(|| claims.last().map(|claim| claim.claim_fence.clone()))
            .flatten();
        let permitted_recovery_paths = if instance.is_ended() {
            vec![Action::NoFurtherWork]
        } else if deadline_overdue {
            vec![Action::EnforceDeadlines]
        } else if instance.lifecycle().is_paused() {
            vec![Action::ResumeAdmission, Action::InspectExternalEffects]
        } else {
            vec![Action::InspectExternalEffects]
        };
        let engine_drained = instance
            .step_records()
            .values()
            .all(|record| record.status() != StepStatus::InProgress);
        Ok(CeremonyResumePreflight {
            ceremony_id: input.ceremony_id,
            journal_version: session.version,
            inspected_at: now,
            lifecycle: instance.lifecycle().phase(),
            admission_paused: instance.lifecycle().is_paused(),
            engine_drained,
            all_claims_host_reported_quiesced,
            coordinated_resume_ready: instance.lifecycle().is_paused()
                && (engine_drained || all_claims_host_reported_quiesced)
                && !deadline_overdue
                && instance.step_records().values().all(|record| {
                    record.status() != StepStatus::InProgress || record.has_live_lease_at(now)
                }),
            ceremony_deadline_at,
            state_deadline_at,
            deadline_overdue,
            claims,
            next_after_claim,
            permitted_recovery_paths,
        })
    }

    async fn claim(
        &self,
        instance: &CeremonyInstance,
        records: &[AuditRecord],
        started: &StepStarted,
        now: time::OffsetDateTime,
        absolute_deadline_overdue: bool,
    ) -> Result<CeremonyClaimPreflight, DomainError> {
        let fence = started.claim_fence(instance.id())?;
        let expires_at = effective_expiry(records, started, &fence);
        let phase = claim_phase(instance, records, started, &fence, expires_at, now);
        let operation_id = ExecutionOperationId::for_step(
            instance.id(),
            &started.step_id,
            started.state_visit(),
            started.state_iteration(),
            started.iteration,
        );
        let execution = self.execution(instance, &operation_id, &fence).await?;
        let step_deadline_at = started.deadline.as_ref().map(StepDeadline::at);
        let deadline_overdue = absolute_deadline_overdue
            || (is_in_flight_claim(instance, started, &fence)
                && step_deadline_at.is_some_and(|at| at <= now));
        let permitted_recovery_paths =
            permitted_paths(phase, instance.is_ended(), deadline_overdue, &execution);
        Ok(CeremonyClaimPreflight {
            step_id: started.step_id.clone(),
            claim_fence: fence.clone(),
            owner: started.lease.owner_id().clone(),
            operation_id,
            phase,
            effective_lease_expires_at: expires_at,
            remaining_lease_ms: u64::try_from((expires_at - now).whole_milliseconds().max(0))
                .unwrap_or(u64::MAX),
            step_deadline_at,
            deadline_overdue,
            budget_reservation_id: started.budget_reservation_id.clone(),
            host_declaration: latest_handoff(records, &fence).cloned(),
            execution,
            permitted_recovery_paths,
        })
    }

    async fn imported_claim(
        &self,
        instance: &CeremonyInstance,
        records: &[AuditRecord],
        imported: &ImportedClaim,
        now: time::OffsetDateTime,
        absolute_deadline_overdue: bool,
    ) -> Result<CeremonyClaimPreflight, DomainError> {
        let step_id = &imported.step_id;
        let record = &imported.record;
        let lease = record.lease().ok_or(DomainError::NotFound {
            what: "imported_step_lease",
        })?;
        let fence = imported.fence.clone();
        let imported_expiry = record
            .effective_lease_expires_at()
            .ok_or(DomainError::NotFound {
                what: "imported_step_lease_expiry",
            })?;
        // Renewals belong to a fence, not to whatever claim later occupies the
        // step. Fold only the last sealed renewal of this imported identity.
        let expires_at = records
            .iter()
            .rev()
            .find_map(|audit| match audit.event() {
                Some(CeremonyEvent::StepLeaseRenewed(renewed)) if renewed.claim_fence == fence => {
                    Some(renewed.expires_at)
                }
                _ => None,
            })
            .unwrap_or(imported_expiry);
        let phase =
            imported_claim_phase(instance, records, step_id, record, &fence, expires_at, now);
        let operation_id = ExecutionOperationId::for_step(
            instance.id(),
            step_id,
            record.state_visit(),
            record.state_iteration(),
            record.iteration(),
        );
        let execution = self.execution(instance, &operation_id, &fence).await?;
        let step_deadline_at = imported
            .deadline
            .as_ref()
            .and_then(|deadline| (deadline.claim_fence() == &fence).then_some(deadline.at()));
        let deadline_overdue = absolute_deadline_overdue
            || ((phase == super::CeremonyClaimPhase::Live
                || phase == super::CeremonyClaimPhase::Expired)
                && step_deadline_at.is_some_and(|at| at <= now));
        let permitted_recovery_paths =
            permitted_paths(phase, instance.is_ended(), deadline_overdue, &execution);
        Ok(CeremonyClaimPreflight {
            step_id: step_id.clone(),
            claim_fence: fence.clone(),
            owner: lease.owner_id().clone(),
            operation_id,
            phase,
            effective_lease_expires_at: expires_at,
            remaining_lease_ms: u64::try_from((expires_at - now).whole_milliseconds().max(0))
                .unwrap_or(u64::MAX),
            step_deadline_at,
            deadline_overdue,
            budget_reservation_id: record.budget_reservation_id().cloned(),
            host_declaration: latest_handoff(records, &fence).cloned(),
            execution,
            permitted_recovery_paths,
        })
    }

    async fn execution(
        &self,
        instance: &CeremonyInstance,
        operation: &ExecutionOperationId,
        fence: &StepClaimFence,
    ) -> Result<ClaimExecutionEvidence, DomainError> {
        let intent = self.receipts.intent(operation, fence).await?;
        let receipt = self.receipts.receipt(operation).await?;
        let reconciliation = self
            .receipts
            .reconciliation_requirement(operation, fence)
            .await?;
        Ok(ClaimExecutionEvidence {
            intent_recorded: intent.is_some(),
            receipt_id: receipt.as_ref().map(|r| r.receipt_id().clone()),
            receipt_producer_fence: receipt.as_ref().map(|r| r.producer_claim_fence().clone()),
            receipt_status: receipt.as_ref().map(|r| r.result().status()),
            external_operation_id: receipt
                .as_ref()
                .and_then(|r| r.external_operation_id().cloned()),
            recovery_capability: receipt
                .as_ref()
                .map(ExecutionReceipt::recovery_capability)
                .or_else(|| intent.as_ref().map(ExecutionIntent::recovery_capability)),
            receipt_applied: instance.execution_receipt_link(operation).is_some(),
            reconciliation_required: reconciliation.is_some(),
        })
    }
}
