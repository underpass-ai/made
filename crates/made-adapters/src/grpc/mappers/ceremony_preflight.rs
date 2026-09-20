use super::host_handoff::{handoff_to_proto, label, moment};
use made_app::workers::{CeremonyClaimPreflight, CeremonyResumePreflight};
use made_proto::v1 as pb;

pub(crate) fn preflight_to_proto(report: CeremonyResumePreflight) -> pb::CeremonyResumePreflight {
    pb::CeremonyResumePreflight {
        ceremony_id: report.ceremony_id.as_str().into(),
        journal_version: report.journal_version.value(),
        inspected_at: moment(report.inspected_at),
        lifecycle: label(report.lifecycle),
        admission_paused: report.admission_paused,
        engine_drained: report.engine_drained,
        all_claims_host_reported_quiesced: report.all_claims_host_reported_quiesced,
        coordinated_resume_ready: report.coordinated_resume_ready,
        ceremony_deadline_at: report.ceremony_deadline_at.map(moment).unwrap_or_default(),
        state_deadline_at: report.state_deadline_at.map(moment).unwrap_or_default(),
        deadline_overdue: report.deadline_overdue,
        claims: report.claims.into_iter().map(claim_to_proto).collect(),
        next_after_claim: report
            .next_after_claim
            .map(|f| f.as_str().to_owned())
            .unwrap_or_default(),
        permitted_recovery_paths: report
            .permitted_recovery_paths
            .into_iter()
            .map(label)
            .collect(),
    }
}

fn claim_to_proto(claim: CeremonyClaimPreflight) -> pb::CeremonyClaimPreflight {
    let e = claim.execution;
    pb::CeremonyClaimPreflight {
        step_id: claim.step_id.as_str().into(),
        claim_fence: claim.claim_fence.as_str().into(),
        owner: claim.owner.as_str().into(),
        operation_id: claim.operation_id.as_str().into(),
        phase: label(claim.phase),
        effective_lease_expires_at: moment(claim.effective_lease_expires_at),
        remaining_lease_ms: claim.remaining_lease_ms,
        step_deadline_at: claim.step_deadline_at.map(moment).unwrap_or_default(),
        deadline_overdue: claim.deadline_overdue,
        budget_reservation_id: claim
            .budget_reservation_id
            .map(|id| id.as_str().to_owned())
            .unwrap_or_default(),
        host_declaration: claim.host_declaration.map(handoff_to_proto),
        execution: Some(pb::ClaimExecutionEvidence {
            intent_recorded: e.intent_recorded,
            receipt_id: e
                .receipt_id
                .map(|id| id.as_str().to_owned())
                .unwrap_or_default(),
            receipt_producer_fence: e
                .receipt_producer_fence
                .map(|f| f.as_str().to_owned())
                .unwrap_or_default(),
            receipt_status: e.receipt_status.map(label).unwrap_or_default(),
            external_operation_id: e
                .external_operation_id
                .map(|id| id.as_str().to_owned())
                .unwrap_or_default(),
            recovery_capability: e.recovery_capability.map(label).unwrap_or_default(),
            receipt_applied: e.receipt_applied,
            reconciliation_required: e.reconciliation_required,
        }),
        permitted_recovery_paths: claim
            .permitted_recovery_paths
            .into_iter()
            .map(label)
            .collect(),
    }
}
