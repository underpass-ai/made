use made_adapters::sqlite::SqliteCeremonyStore;
use made_app::services::CeremonyTraceScope;
use made_app::usecases::*;
use made_app::workers::*;
use made_core::entities::ceremony_events::InstanceImported;
use made_core::entities::{AuditChain, AuditFact, CeremonyEvent, CeremonyInstance};
use made_core::ports::{CeremonyEventStorePort, ClockPort};
use made_core::value_objects::*;
use made_embedded::EmbeddedMade;
use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};
use time::OffsetDateTime;

const YAML: &str = r#"
version: "1.0"
name: host_preflight
states:
  - {id: WORK, initial: true}
  - {id: DONE, terminal: true}
transitions:
  - {from: WORK, to: DONE, trigger: finish}
steps:
  - {id: work, state: WORK, handler: host_callback}
roles:
  - {id: WORKER, allowed_actions: [work, finish]}
retry_policies:
  default: {max_attempts: 3, backoff_seconds: 0}
"#;

const LEGACY_YAML: &str = r#"
version: "1.0"
name: legacy_static
states:
  - {id: OPEN, initial: true}
steps:
  - {id: draft, state: OPEN, handler: host_callback}
roles:
  - {id: LEGACY, allowed_actions: [draft]}
retry_policies:
  default: {max_attempts: 3, backoff_seconds: 0}
"#;

const SIBLING_DEADLINES_YAML: &str = r#"
version: "1.0"
name: sibling_deadlines
max_parallel: 2
states: [{id: WORK, initial: true, execution: concurrent}, {id: DONE, terminal: true}]
transitions: [{from: WORK, to: DONE, trigger: finish, guards: [both_done]}]
steps:
  - {id: work, state: WORK, handler: host_callback}
  - {id: peer, state: WORK, handler: host_callback}
roles:
  - {id: WORKER, allowed_actions: [work]}
  - {id: PEER, allowed_actions: [peer]}
guards: {both_done: {type: automated, check: "steps_completed:2"}}
retry_policies: {default: {max_attempts: 3, backoff_seconds: 0}}
timeouts: {step_default: 3}
"#;

#[derive(Default)]
struct Clock(AtomicI64);
impl ClockPort for Clock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(self.0.load(Ordering::SeqCst)).unwrap()
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    engine: EmbeddedMade,
    clock: Arc<Clock>,
    path: std::path::PathBuf,
}
impl Fixture {
    async fn new(timeout: Option<&str>) -> Self {
        let scratch =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/host-handoff-188");
        std::fs::create_dir_all(&scratch).unwrap();
        let dir = tempfile::tempdir_in(scratch).unwrap();
        let path = dir.path().join("store.sqlite3");
        let clock = Arc::new(Clock::default());
        let engine = EmbeddedMade::builder()
            .with_ceremony_store(Arc::new(SqliteCeremonyStore::open(&path).unwrap()))
            .with_clock(clock.clone())
            .build();
        let yaml = timeout.map_or_else(
            || YAML.to_owned(),
            |kind| format!("{YAML}\ntimeouts:\n  {kind}: 3\n"),
        );
        let definition = engine.mount_yaml(&yaml).await.unwrap().definitions()[0].clone();
        engine
            .start(StartCeremonyInput::new(
                id(),
                definition.name().clone(),
                definition.version().clone(),
                CeremonyContext::empty(),
                "operator",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();
        Self {
            _dir: dir,
            engine,
            clock,
            path,
        }
    }
    async fn claim(&self, owner: &str) -> StepClaimFence {
        self.engine
            .start_step(StartCeremonyStepInput::new(
                id(),
                RoleId::new("WORKER").unwrap(),
                AuditActorKind::Agent,
                step(),
                LeaseOwnerId::new(owner).unwrap(),
                IdempotencyKey::new(owner).unwrap(),
                DurationMs::from_millis(1000),
            ))
            .await
            .unwrap()
            .claim_fence()
            .clone()
    }
    async fn pause(&self) {
        self.engine
            .pause_ceremony(PauseCeremonyInput::new(
                id(),
                "operator",
                AuditActorKind::Service,
                LifecycleReason::new("host handoff").unwrap(),
            ))
            .await
            .unwrap();
    }
    async fn report(&self) -> CeremonyResumePreflight {
        self.engine
            .inspect_ceremony_resume(query(None, 100))
            .await
            .unwrap()
    }
}
fn id() -> CeremonyId {
    CeremonyId::new("handoff").unwrap()
}
fn step() -> StepId {
    StepId::new("work").unwrap()
}
fn query(after_claim: Option<StepClaimFence>, limit: u16) -> InspectCeremonyResumeInput {
    InspectCeremonyResumeInput {
        ceremony_id: id(),
        after_claim,
        limit: ExecutionRecoveryPageLimit::new(limit).unwrap(),
    }
}
fn declaration(fence: &StepClaimFence, state: HostWorkState) -> RecordCeremonyHostHandoffInput {
    RecordCeremonyHostHandoffInput {
        ceremony_id: id(),
        declaration: HostHandoffDeclaration {
            id: IdempotencyKey::new("handoff-1").unwrap(),
            step_id: step(),
            claim_fence: fence.clone(),
            owner: LeaseOwnerId::new("host-a").unwrap(),
            incarnation: HostAgentIncarnation::new("task-1").unwrap(),
            state,
            observed_at: OffsetDateTime::UNIX_EPOCH,
            evidence: EvidenceReference::new("artifact:handoff-transcript-1").unwrap(),
        },
    }
}
fn completion(fence: &StepClaimFence) -> CompleteCeremonyStepInput {
    CompleteCeremonyStepInput::new(
        id(),
        step(),
        StepResult::completed(StepOutput::empty()).unwrap(),
        AuditActorKind::Agent,
        fence.clone(),
    )
}

#[tokio::test]
async fn pause_never_infers_quiescence_and_durable_exact_ack_is_idempotent() {
    let f = Fixture::new(None).await;
    let fence = f.claim("host-a").await;
    f.pause().await;
    let before = f.report().await;
    assert!(before.admission_paused);
    assert!(!before.engine_drained);
    assert!(!before.all_claims_host_reported_quiesced);
    assert!(!before.coordinated_resume_ready);
    assert_eq!(before.claims[0].phase, CeremonyClaimPhase::Live);
    let input = declaration(&fence, HostWorkState::Quiesced);
    let (a, b) = tokio::join!(
        f.engine.record_ceremony_host_handoff(input.clone()),
        f.engine.record_ceremony_host_handoff(input.clone())
    );
    let accepted = a.unwrap();
    assert_eq!(accepted, b.unwrap());
    let after = f.report().await;
    assert!(after.all_claims_host_reported_quiesced);
    assert!(after.coordinated_resume_ready);
    assert!(
        !after.engine_drained,
        "host evidence does not complete engine work"
    );
    assert_eq!(
        after.claims[0].effective_lease_expires_at,
        before.claims[0].effective_lease_expires_at
    );
    let records = f.engine.audit_records(&id()).await.unwrap();
    assert_eq!(
        records
            .iter()
            .filter(|r| r.event_type() == AuditEventType::HostHandoffRecorded)
            .count(),
        1
    );
    assert!(AuditChain::verify(&records).is_intact());
    let reopened = EmbeddedMade::builder()
        .with_ceremony_store(Arc::new(SqliteCeremonyStore::open(&f.path).unwrap()))
        .with_clock(f.clock.clone())
        .build();
    reopened.mount_yaml(YAML).await.unwrap();
    assert_eq!(
        reopened.record_ceremony_host_handoff(input).await.unwrap(),
        accepted
    );
    assert_eq!(
        reopened
            .inspect_ceremony_resume(query(None, 100))
            .await
            .unwrap(),
        after
    );
}

#[tokio::test]
async fn record_handoff_seals_the_callers_traceparent_on_its_audit_event() {
    let f = Fixture::new(None).await;
    let fence = f.claim("host-a").await;
    let trace =
        TraceContext::parse("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01").unwrap();
    CeremonyTraceScope::run(
        trace.clone(),
        f.engine
            .record_ceremony_host_handoff(declaration(&fence, HostWorkState::Quiesced)),
    )
    .await
    .unwrap();
    let record = f.engine.audit_records(&id()).await.unwrap().pop().unwrap();
    assert_eq!(record.event_type(), AuditEventType::HostHandoffRecorded);
    assert_eq!(record.trace_id(), Some(trace.trace_id()));
}

async fn legacy_imported_engine() -> (EmbeddedMade, CeremonyId, OffsetDateTime) {
    let scratch =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/host-handoff-188");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let store =
        Arc::new(SqliteCeremonyStore::open(directory.path().join("store.sqlite3")).unwrap());
    let snapshot: CeremonyInstance = serde_json::from_str(include_str!(
        "../../made-core/tests/fixtures/legacy_in_progress_instance_pre_p5.json"
    ))
    .unwrap();
    let ceremony_id = snapshot.id().clone();
    let imported_at = time::macros::datetime!(2026-07-29 09:10:00 UTC);
    let event = CeremonyEvent::InstanceImported(InstanceImported {
        ceremony_id: ceremony_id.clone(),
        definition_name: snapshot.definition_name().clone(),
        definition_version: snapshot.definition_version().clone(),
        snapshot: Box::new(snapshot),
        legacy_journal_head_hash: None,
        legacy_revision: CeremonyRevision::INITIAL,
        imported_at,
    });
    store
        .append(
            &ceremony_id,
            StreamVersion::EMPTY,
            vec![AuditFact {
                event_id: EventId::new("legacy-active:instance-imported:1").unwrap(),
                ceremony_id: ceremony_id.clone(),
                definition_name: CeremonyName::new("legacy_static").unwrap(),
                definition_version: CeremonyVersion::v1(),
                occurred_at: imported_at,
                actor: AuditActor::engine("made-migration").unwrap(),
                correlation_id: None,
                causation_id: None,
                trace: None,
                event,
            }],
        )
        .await
        .unwrap();
    let clock = Arc::new(Clock(AtomicI64::new(imported_at.unix_timestamp())));
    let engine = EmbeddedMade::builder()
        .with_ceremony_store(store)
        .with_clock(clock)
        .build();
    engine.mount_yaml(LEGACY_YAML).await.unwrap();
    (engine, ceremony_id, imported_at)
}

#[tokio::test]
async fn imported_legacy_in_progress_claim_is_preflighted_and_accepts_a_durable_handoff() {
    let (engine, ceremony_id, imported_at) = legacy_imported_engine().await;
    let report = engine
        .inspect_ceremony_resume(InspectCeremonyResumeInput {
            ceremony_id: ceremony_id.clone(),
            after_claim: None,
            limit: ExecutionRecoveryPageLimit::DEFAULT,
        })
        .await
        .unwrap();
    assert_eq!(report.claims.len(), 1);
    let claim = &report.claims[0];
    assert_eq!(claim.owner.as_str(), "legacy-host");
    assert_eq!(claim.phase, CeremonyClaimPhase::Expired);
    assert!(claim
        .permitted_recovery_paths
        .contains(&CeremonyPreflightAction::ReclaimAfterReconciliationWithNewFence));
    let recorded = engine
        .record_ceremony_host_handoff(RecordCeremonyHostHandoffInput {
            ceremony_id: ceremony_id.clone(),
            declaration: HostHandoffDeclaration {
                id: IdempotencyKey::new("legacy-handoff-1").unwrap(),
                step_id: StepId::new("draft").unwrap(),
                claim_fence: claim.claim_fence.clone(),
                owner: claim.owner.clone(),
                incarnation: HostAgentIncarnation::new("legacy-task-1").unwrap(),
                state: HostWorkState::Quiesced,
                observed_at: imported_at,
                evidence: EvidenceReference::new("artifact:legacy-handoff").unwrap(),
            },
        })
        .await
        .unwrap();
    assert_eq!(recorded.declaration.claim_fence, claim.claim_fence);
    let replacement = engine
        .start_step(StartCeremonyStepInput::new(
            ceremony_id.clone(),
            RoleId::new("LEGACY").unwrap(),
            AuditActorKind::Agent,
            StepId::new("draft").unwrap(),
            LeaseOwnerId::new("replacement-host").unwrap(),
            IdempotencyKey::new("legacy-reclaim").unwrap(),
            DurationMs::from_millis(60_000),
        ))
        .await
        .unwrap();
    let first_page = engine
        .inspect_ceremony_resume(InspectCeremonyResumeInput {
            ceremony_id: ceremony_id.clone(),
            after_claim: None,
            limit: ExecutionRecoveryPageLimit::new(1).unwrap(),
        })
        .await
        .unwrap();
    let second_page = engine
        .inspect_ceremony_resume(InspectCeremonyResumeInput {
            ceremony_id: ceremony_id.clone(),
            after_claim: first_page.next_after_claim.clone(),
            limit: ExecutionRecoveryPageLimit::new(1).unwrap(),
        })
        .await
        .unwrap();
    let claims = [first_page.claims[0].clone(), second_page.claims[0].clone()];
    let original = claims
        .iter()
        .find(|candidate| candidate.claim_fence == claim.claim_fence)
        .unwrap();
    let current = claims
        .iter()
        .find(|candidate| candidate.claim_fence == *replacement.claim_fence())
        .unwrap();
    assert_eq!(original.phase, CeremonyClaimPhase::Retired);
    assert_eq!(original.owner.as_str(), "legacy-host");
    assert_eq!(original.host_declaration.as_ref(), Some(&recorded));
    assert_ne!(current.host_declaration.as_ref(), Some(&recorded));
    assert!(
        !first_page.all_claims_host_reported_quiesced,
        "only B is active"
    );
    assert!(engine
        .audit_records(&ceremony_id)
        .await
        .unwrap()
        .iter()
        .any(|record| record.event_type() == AuditEventType::HostHandoffRecorded));
}

#[tokio::test]
async fn changed_payload_incarnation_owner_and_future_time_refuse_without_mutation() {
    let f = Fixture::new(None).await;
    let fence = f.claim("host-a").await;
    let input = declaration(&fence, HostWorkState::HandoffAcknowledged);
    f.engine
        .record_ceremony_host_handoff(input.clone())
        .await
        .unwrap();
    let before = f.engine.audit_records(&id()).await.unwrap();
    let mut variants = Vec::new();
    let mut changed = input.clone();
    changed.declaration.state = HostWorkState::Quiesced;
    variants.push(changed);
    let mut changed = input.clone();
    changed.declaration.id = IdempotencyKey::new("new-id").unwrap();
    changed.declaration.incarnation = HostAgentIncarnation::new("replacement-task").unwrap();
    variants.push(changed);
    let mut changed = input.clone();
    changed.declaration.owner = LeaseOwnerId::new("host-b").unwrap();
    variants.push(changed);
    let mut changed = input.clone();
    changed.declaration.observed_at += time::Duration::SECOND;
    variants.push(changed);
    let mut changed = input;
    changed.declaration.evidence = EvidenceReference::new("x".repeat(2049)).unwrap();
    variants.push(changed);
    for rejected in variants {
        assert!(f
            .engine
            .record_ceremony_host_handoff(rejected)
            .await
            .is_err());
        assert_eq!(f.engine.audit_records(&id()).await.unwrap(), before);
    }
}

#[tokio::test]
async fn contradictory_handoff_at_the_same_observation_time_cannot_fabricate_readiness() {
    let f = Fixture::new(None).await;
    let fence = f.claim("host-a").await;
    f.pause().await;
    let lost = declaration(&fence, HostWorkState::Lost);
    f.engine
        .record_ceremony_host_handoff(lost.clone())
        .await
        .unwrap();
    let before_records = f.engine.audit_records(&id()).await.unwrap();
    let before_report = f.report().await;
    assert!(!before_report.all_claims_host_reported_quiesced);
    assert!(!before_report.coordinated_resume_ready);

    let mut contradictory = lost;
    contradictory.declaration.id = IdempotencyKey::new("handoff-2").unwrap();
    contradictory.declaration.state = HostWorkState::Quiesced;
    assert!(f
        .engine
        .record_ceremony_host_handoff(contradictory)
        .await
        .is_err());
    assert_eq!(f.engine.audit_records(&id()).await.unwrap(), before_records);
    assert_eq!(f.report().await, before_report);
}

#[tokio::test]
async fn engine_drained_makes_paused_resume_ready_without_host_acknowledgement() {
    let f = Fixture::new(None).await;
    let fence = f.claim("host-a").await;
    f.pause().await;
    f.engine.complete_step(completion(&fence)).await.unwrap();
    let report = f.report().await;
    assert!(report.admission_paused && report.engine_drained);
    assert!(!report.all_claims_host_reported_quiesced);
    assert!(
        report.coordinated_resume_ready,
        "sealed completion drains the engine without inferring a host acknowledgement"
    );
    assert_eq!(report.claims[0].phase, CeremonyClaimPhase::Completed);
}

#[tokio::test]
async fn expired_inflight_claim_still_needs_recovery_after_a_quiesced_acknowledgement() {
    let f = Fixture::new(None).await;
    let fence = f.claim("host-a").await;
    f.pause().await;
    f.engine
        .record_ceremony_host_handoff(declaration(&fence, HostWorkState::Quiesced))
        .await
        .unwrap();
    f.clock.0.store(2, Ordering::SeqCst);

    let report = f.report().await;
    assert!(!report.engine_drained);
    assert!(report.all_claims_host_reported_quiesced);
    assert!(
        !report.coordinated_resume_ready,
        "an expired in-flight claim needs explicit recovery despite its host acknowledgement"
    );
    assert_eq!(report.claims[0].phase, CeremonyClaimPhase::Expired);
    assert!(report.claims[0]
        .permitted_recovery_paths
        .contains(&CeremonyPreflightAction::ReclaimAfterReconciliationWithNewFence));
}

#[tokio::test]
async fn host_loss_expiry_replacement_and_pagination_preserve_operation_and_original_fences() {
    let f = Fixture::new(None).await;
    let original = f.claim("host-a").await;
    let original_receipt = f
        .engine
        .record_ceremony_host_handoff(declaration(&original, HostWorkState::Lost))
        .await
        .unwrap();
    f.pause().await;
    f.clock.0.store(2, Ordering::SeqCst);
    let expired = f.report().await;
    assert_eq!(expired.claims[0].phase, CeremonyClaimPhase::Expired);
    assert_eq!(expired.claims[0].remaining_lease_ms, 0);
    assert!(!expired.all_claims_host_reported_quiesced);
    assert!(!expired.coordinated_resume_ready);
    assert!(expired.claims[0]
        .permitted_recovery_paths
        .contains(&CeremonyPreflightAction::InspectExternalEffects));
    f.engine
        .resume_ceremony(ResumeCeremonyInput::new(
            id(),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    let replacement = f.claim("host-b").await;
    let before = f.engine.audit_records(&id()).await.unwrap();
    assert_eq!(
        f.engine
            .record_ceremony_host_handoff(declaration(&original, HostWorkState::Lost))
            .await
            .unwrap(),
        original_receipt,
        "an exact durable replay returns the original receipt without reviving its fence"
    );
    let mut altered_replay = declaration(&original, HostWorkState::Lost);
    altered_replay.declaration.evidence = EvidenceReference::new("artifact:changed").unwrap();
    assert!(f
        .engine
        .record_ceremony_host_handoff(altered_replay)
        .await
        .is_err());
    assert!(f.engine.complete_step(completion(&original)).await.is_err());
    assert_eq!(f.engine.audit_records(&id()).await.unwrap(), before);
    let first = f
        .engine
        .inspect_ceremony_resume(query(None, 1))
        .await
        .unwrap();
    assert_eq!(
        first,
        f.engine
            .inspect_ceremony_resume(query(None, 1))
            .await
            .unwrap()
    );
    let second = f
        .engine
        .inspect_ceremony_resume(query(first.next_after_claim.clone(), 1))
        .await
        .unwrap();
    assert_eq!(first.journal_version, second.journal_version);
    assert!(second.next_after_claim.is_none());
    assert_eq!(first.claims[0].phase, CeremonyClaimPhase::Retired);
    assert_eq!(second.claims[0].phase, CeremonyClaimPhase::Live);
    assert_eq!(first.claims[0].operation_id, second.claims[0].operation_id);
    assert_eq!(first.claims[0].claim_fence, original);
    assert_eq!(second.claims[0].claim_fence, replacement);
    f.pause().await;
    let mut replacement_handoff = declaration(&replacement, HostWorkState::Quiesced);
    replacement_handoff.declaration.id = IdempotencyKey::new("replacement-handoff").unwrap();
    replacement_handoff.declaration.owner = LeaseOwnerId::new("host-b").unwrap();
    replacement_handoff.declaration.observed_at = OffsetDateTime::from_unix_timestamp(2).unwrap();
    f.engine
        .record_ceremony_host_handoff(replacement_handoff)
        .await
        .unwrap();
    let coordinated = f.report().await;
    assert!(
        coordinated.all_claims_host_reported_quiesced,
        "the retired historical fence cannot block the current quiesced claim"
    );
    assert!(coordinated.coordinated_resume_ready);
    assert!(!coordinated.engine_drained);
    f.engine
        .complete_step(completion(&replacement))
        .await
        .unwrap();
}

#[tokio::test]
async fn long_pauses_report_absolute_deadlines_without_moving_any_clock() {
    for timeout in ["step_default", "state_default", "ceremony"] {
        let f = Fixture::new(Some(timeout)).await;
        f.claim("host-a").await;
        f.pause().await;
        let before = f.report().await;
        f.clock.0.store(4, Ordering::SeqCst);
        let after = f.report().await;
        assert_eq!(
            before.claims[0].effective_lease_expires_at,
            after.claims[0].effective_lease_expires_at
        );
        assert_eq!(
            before.claims[0].step_deadline_at,
            after.claims[0].step_deadline_at
        );
        assert_eq!(before.state_deadline_at, after.state_deadline_at);
        assert_eq!(before.ceremony_deadline_at, after.ceremony_deadline_at);
        assert!(after.claims[0].deadline_overdue);
        assert!(after.claims[0]
            .permitted_recovery_paths
            .contains(&CeremonyPreflightAction::EnforceDeadlines));
    }
}

#[tokio::test]
async fn deadline_overdue_is_claim_local_and_clears_after_completion_or_deadline_retirement() {
    let completed = Fixture::new(Some("step_default")).await;
    let completed_fence = completed.claim("host-a").await;
    completed.clock.0.store(1, Ordering::SeqCst);
    completed
        .engine
        .complete_step(completion(&completed_fence))
        .await
        .unwrap();
    completed.clock.0.store(3, Ordering::SeqCst);
    let completed_report = completed.report().await;
    assert!(!completed_report.claims[0].deadline_overdue);
    assert!(!completed_report.claims[0]
        .permitted_recovery_paths
        .contains(&CeremonyPreflightAction::EnforceDeadlines));

    let expired = Fixture::new(Some("step_default")).await;
    expired.claim("host-a").await;
    expired.clock.0.store(3, Ordering::SeqCst);
    let before = expired.report().await;
    assert!(before.claims[0].deadline_overdue);
    assert!(before.claims[0]
        .permitted_recovery_paths
        .contains(&CeremonyPreflightAction::EnforceDeadlines));
    expired
        .engine
        .enforce_ceremony_deadlines(EnforceCeremonyDeadlinesInput::new(id()))
        .await
        .unwrap();
    expired
        .engine
        .enforce_ceremony_deadlines(EnforceCeremonyDeadlinesInput::new(id()))
        .await
        .unwrap();
    let retired = expired.report().await;
    assert!(!retired.claims[0].deadline_overdue);
    assert!(!retired.claims[0]
        .permitted_recovery_paths
        .contains(&CeremonyPreflightAction::EnforceDeadlines));
    assert_eq!(
        expired
            .engine
            .audit_records(&id())
            .await
            .unwrap()
            .iter()
            .filter(|record| record.event_type() == AuditEventType::StepDeadlineExceeded)
            .count(),
        1
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Exercises every page and each independent claim cleanup.
async fn paginated_sibling_deadlines_block_admission_until_each_active_claim_is_cleared() {
    let scratch =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/host-handoff-188");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let clock = Arc::new(Clock::default());
    let engine = EmbeddedMade::builder()
        .with_ceremony_store(Arc::new(
            SqliteCeremonyStore::open(directory.path().join("siblings.sqlite3")).unwrap(),
        ))
        .with_clock(clock.clone())
        .build();
    let definition = engine
        .mount_yaml(SIBLING_DEADLINES_YAML)
        .await
        .unwrap()
        .definitions()[0]
        .clone();
    let ceremony_id = CeremonyId::new("sibling-deadlines").unwrap();
    engine
        .start(StartCeremonyInput::new(
            ceremony_id.clone(),
            definition.name().clone(),
            definition.version().clone(),
            CeremonyContext::empty(),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .unwrap();
    let claim = |role: &str, step: &str, owner: &str| {
        StartCeremonyStepInput::new(
            ceremony_id.clone(),
            RoleId::new(role).unwrap(),
            AuditActorKind::Agent,
            StepId::new(step).unwrap(),
            LeaseOwnerId::new(owner).unwrap(),
            IdempotencyKey::new(format!("{owner}-claim")).unwrap(),
            DurationMs::from_millis(60_000),
        )
    };
    let work = engine
        .start_step(claim("WORKER", "work", "work-host"))
        .await
        .unwrap();
    let peer = engine
        .start_step(claim("PEER", "peer", "peer-host"))
        .await
        .unwrap();
    engine
        .pause_ceremony(PauseCeremonyInput::new(
            ceremony_id.clone(),
            "operator",
            AuditActorKind::Service,
            LifecycleReason::new("handoff").unwrap(),
        ))
        .await
        .unwrap();
    for (id, step_id, owner, fence) in [
        ("work-ack", "work", "work-host", work.claim_fence().clone()),
        ("peer-ack", "peer", "peer-host", peer.claim_fence().clone()),
    ] {
        engine
            .record_ceremony_host_handoff(RecordCeremonyHostHandoffInput {
                ceremony_id: ceremony_id.clone(),
                declaration: HostHandoffDeclaration {
                    id: IdempotencyKey::new(id).unwrap(),
                    step_id: StepId::new(step_id).unwrap(),
                    claim_fence: fence,
                    owner: LeaseOwnerId::new(owner).unwrap(),
                    incarnation: HostAgentIncarnation::new(format!("{owner}-inc")).unwrap(),
                    state: HostWorkState::Quiesced,
                    observed_at: OffsetDateTime::UNIX_EPOCH,
                    evidence: EvidenceReference::new(format!("artifact:{id}")).unwrap(),
                },
            })
            .await
            .unwrap();
    }
    clock.0.store(4, Ordering::SeqCst);
    let first = engine
        .inspect_ceremony_resume(InspectCeremonyResumeInput {
            ceremony_id: ceremony_id.clone(),
            after_claim: None,
            limit: ExecutionRecoveryPageLimit::new(1).unwrap(),
        })
        .await
        .unwrap();
    let second = engine
        .inspect_ceremony_resume(InspectCeremonyResumeInput {
            ceremony_id: ceremony_id.clone(),
            after_claim: first.next_after_claim.clone(),
            limit: ExecutionRecoveryPageLimit::new(1).unwrap(),
        })
        .await
        .unwrap();
    for page in [&first, &second] {
        assert!(page.deadline_overdue);
        assert!(!page.coordinated_resume_ready);
        assert!(!page
            .permitted_recovery_paths
            .contains(&CeremonyPreflightAction::ResumeAdmission));
    }
    engine
        .complete_step(CompleteCeremonyStepInput::new(
            ceremony_id.clone(),
            StepId::new("work").unwrap(),
            StepResult::completed(StepOutput::empty()).unwrap(),
            AuditActorKind::Agent,
            work.claim_fence().clone(),
        ))
        .await
        .unwrap();
    let one_remains = engine
        .inspect_ceremony_resume(InspectCeremonyResumeInput {
            ceremony_id: ceremony_id.clone(),
            after_claim: None,
            limit: ExecutionRecoveryPageLimit::DEFAULT,
        })
        .await
        .unwrap();
    assert!(
        one_remains.deadline_overdue,
        "the peer must retain its own deadline barrier"
    );
    engine
        .complete_step(CompleteCeremonyStepInput::new(
            ceremony_id.clone(),
            StepId::new("peer").unwrap(),
            StepResult::completed(StepOutput::empty()).unwrap(),
            AuditActorKind::Agent,
            peer.claim_fence().clone(),
        ))
        .await
        .unwrap();
    let cleared = engine
        .inspect_ceremony_resume(InspectCeremonyResumeInput {
            ceremony_id,
            after_claim: None,
            limit: ExecutionRecoveryPageLimit::DEFAULT,
        })
        .await
        .unwrap();
    assert!(!cleared.deadline_overdue);
}
