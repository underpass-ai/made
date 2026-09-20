use super::super::{CeremonyPreflightAction as Action, ClaimExecutionEvidence};
use made_core::entities::ceremony_events::{HostHandoffRecorded, StepStarted};
use made_core::entities::{AuditRecord, CeremonyEvent, CeremonyInstance};
use made_core::value_objects::CeremonyClaimPhase as Phase;
use made_core::value_objects::{StepClaimFence, StepStatus};
use time::OffsetDateTime;

pub(super) fn latest_handoff<'a>(
    records: &'a [AuditRecord],
    fence: &StepClaimFence,
) -> Option<&'a HostHandoffRecorded> {
    records
        .iter()
        .rev()
        .find_map(|record| match record.event() {
            Some(CeremonyEvent::HostHandoffRecorded(handoff))
                if &handoff.declaration.claim_fence == fence =>
            {
                Some(handoff)
            }
            _ => None,
        })
}

/// A historical `StepStarted` needs host disposition only while its exact
/// fence still names the engine's current in-flight claim. An expired current
/// claim remains in flight here; readiness separately refuses its stale lease.
pub(super) fn is_in_flight_claim(
    instance: &CeremonyInstance,
    started: &StepStarted,
    fence: &StepClaimFence,
) -> bool {
    instance
        .step_record(&started.step_id)
        .is_some_and(|record| {
            record.status() == StepStatus::InProgress
                && instance
                    .step_claim_fence(&started.step_id)
                    .is_ok_and(|current| &current == fence)
        })
}

pub(super) fn effective_expiry(
    records: &[AuditRecord],
    started: &StepStarted,
    fence: &StepClaimFence,
) -> OffsetDateTime {
    records
        .iter()
        .rev()
        .find_map(|record| match record.event() {
            Some(CeremonyEvent::StepLeaseRenewed(renewed)) if &renewed.claim_fence == fence => {
                Some(renewed.expires_at)
            }
            _ => None,
        })
        .unwrap_or_else(|| started.lease.expires_at())
}

pub(super) fn claim_phase(
    instance: &CeremonyInstance,
    records: &[AuditRecord],
    started: &StepStarted,
    fence: &StepClaimFence,
    expiry: OffsetDateTime,
    now: OffsetDateTime,
) -> Phase {
    for record in records.iter().rev() {
        match record.event() {
            Some(CeremonyEvent::StepCompleted(done))
                if done.step_id == started.step_id
                    && done.state_visit() == started.state_visit()
                    && done.state_iteration() == started.state_iteration()
                    && done.iteration == started.iteration
                    && done.attempt == started.attempt =>
            {
                return Phase::Completed
            }
            Some(CeremonyEvent::StepFailed(done))
                if done.step_id == started.step_id
                    && done.state_visit() == started.state_visit()
                    && done.state_iteration() == started.state_iteration()
                    && done.iteration == started.iteration
                    && done.attempt == started.attempt =>
            {
                return Phase::Failed
            }
            _ => {}
        }
    }
    if !instance.is_ended()
        && instance
            .step_claim_fence(&started.step_id)
            .is_ok_and(|current| &current == fence)
    {
        if expiry > now {
            Phase::Live
        } else {
            Phase::Expired
        }
    } else {
        Phase::Retired
    }
}

pub(super) fn permitted_paths(
    phase: Phase,
    ended: bool,
    overdue: bool,
    execution: &ClaimExecutionEvidence,
) -> Vec<Action> {
    if ended || matches!(phase, Phase::Completed | Phase::Failed) {
        return vec![Action::NoFurtherWork, Action::InspectExternalEffects];
    }
    if overdue {
        return vec![Action::EnforceDeadlines, Action::InspectExternalEffects];
    }
    match phase {
        Phase::Live => vec![
            Action::WaitForAcceptedWork,
            Action::RecordHostHandoffEvidence,
            Action::CompleteWithOriginalFence,
        ],
        Phase::Expired | Phase::Retired => {
            let mut paths = vec![Action::InspectExternalEffects];
            if execution.receipt_id.is_some() {
                paths.push(Action::InspectReceiptForAdoption);
            }
            if phase == Phase::Expired {
                paths.push(Action::ReclaimAfterReconciliationWithNewFence);
            }
            paths
        }
        Phase::Completed | Phase::Failed => unreachable!(),
    }
}
