use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

pub(super) fn recorded(record: pb::HostHandoffRecorded) -> Value {
    let declaration = record.declaration.map(|d| json!({
        "id": d.id, "step_id": d.step_id, "claim_fence": d.claim_fence, "owner": d.owner,
        "incarnation": d.incarnation, "state": d.state, "observed_at": d.observed_at, "evidence": d.evidence,
    }));
    json!({"declaration":declaration, "recorded_at":record.recorded_at})
}

fn optional(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

pub(super) fn preflight(report: pb::CeremonyResumePreflight) -> Value {
    json!({
        "ceremony_id":report.ceremony_id, "journal_version":report.journal_version,
        "inspected_at":report.inspected_at, "lifecycle":report.lifecycle,
        "admission_paused":report.admission_paused, "engine_drained":report.engine_drained,
        "all_claims_host_reported_quiesced":report.all_claims_host_reported_quiesced,
        "coordinated_resume_ready":report.coordinated_resume_ready,
        "ceremony_deadline_at":optional(report.ceremony_deadline_at),
        "state_deadline_at":optional(report.state_deadline_at), "deadline_overdue":report.deadline_overdue,
        "next_after_claim":optional(report.next_after_claim),
        "permitted_recovery_paths":report.permitted_recovery_paths,
        "claims":report.claims.into_iter().map(claim).collect::<Vec<_>>(),
    })
}

fn claim(claim: pb::CeremonyClaimPreflight) -> Value {
    let execution = claim.execution.map(|e| json!({
        "intent_recorded":e.intent_recorded, "receipt_id":optional(e.receipt_id),
        "receipt_producer_fence":optional(e.receipt_producer_fence), "receipt_status":optional(e.receipt_status),
        "external_operation_id":optional(e.external_operation_id), "recovery_capability":optional(e.recovery_capability),
        "receipt_applied":e.receipt_applied, "reconciliation_required":e.reconciliation_required,
    }));
    json!({
        "step_id":claim.step_id,"claim_fence":claim.claim_fence,"owner":claim.owner,
        "operation_id":claim.operation_id,"phase":claim.phase,
        "effective_lease_expires_at":claim.effective_lease_expires_at,
        "remaining_lease_ms":claim.remaining_lease_ms,
        "step_deadline_at":optional(claim.step_deadline_at),"deadline_overdue":claim.deadline_overdue,
        "budget_reservation_id":optional(claim.budget_reservation_id),
        "host_declaration":claim.host_declaration.map(recorded),"execution":execution,
        "permitted_recovery_paths":claim.permitted_recovery_paths,
    })
}
