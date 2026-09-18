//! Real work across A -> B -> A, with separate writer processes and snapshot cuts.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use made_adapters::sqlite::SqliteCeremonyStore;
use made_app::services::SessionStream;
use made_app::usecases::{
    ApplyCeremonyTransitionInput, CompleteCeremonyStepInput, StartCeremonyInput,
    StartCeremonyStepInput,
};
use made_core::entities::{CeremonyEvent, CeremonyInstance};
use made_core::ports::{CeremonySnapshot, CeremonySnapshotStorePort, NoopCeremonyEventSubscriber};
use made_core::value_objects::{
    Attributes, AuditActorKind, CeremonyContext, CeremonyId, DurationMs, IdempotencyKey,
    LeaseOwnerId, RoleId, StepId, StepOutput, StepResult, StepStatus, StreamVersion,
    TransitionTrigger,
};
use made_embedded::EmbeddedMade;
use serde_json::json;

const DEFINITION: &str = include_str!("../../../tests/e2e/ceremonies/state-visits.yaml");
const PROCESS_PATH: &str = "MADE_TEST_STATE_VISITS_DB";
const PROCESS_PHASE: &str = "MADE_TEST_STATE_VISITS_PHASE";

fn id() -> CeremonyId {
    CeremonyId::new("durable-visits").unwrap()
}
fn step(raw: &str) -> StepId {
    StepId::new(raw).unwrap()
}
fn role() -> RoleId {
    RoleId::new("DRIVER").unwrap()
}

#[tokio::test]
async fn cyclic_work_survives_process_restart_and_every_snapshot_cut() {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(scratch).unwrap();
    let path = directory.path().join("visits.sqlite3");
    for phase in ["prefix", "tail"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "state_visit_child_process", "--nocapture"])
            .env(PROCESS_PATH, &path)
            .env(PROCESS_PHASE, phase)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let engine = EmbeddedMade::open(&path).unwrap();
    engine.mount_yaml(DEFINITION).await.unwrap();
    let expected = engine.instance(&id()).await.unwrap();
    assert_final(&expected);
    let records = engine.audit_records(&id()).await.unwrap();
    assert_eq!(
        records
            .iter()
            .map(made_core::entities::AuditRecord::event_id)
            .collect::<BTreeSet<_>>()
            .len(),
        records.len()
    );
    assert_eq!(
        SessionStream::fold_records(&records).unwrap().instance,
        expected
    );
    let contributions = engine.transcript(&id()).await.unwrap();
    assert_eq!(
        contributions
            .contributions()
            .iter()
            .map(|entry| (
                entry.step_id().as_str(),
                entry.state_visit().get(),
                entry.state_iteration().get()
            ))
            .collect::<Vec<_>>(),
        [
            ("a", 1, 1),
            ("a", 1, 2),
            ("b", 2, 1),
            ("a", 3, 1),
            ("a", 3, 2)
        ]
    );
    let store = SqliteCeremonyStore::open(&path).unwrap();
    let stream = SessionStream::new(
        Arc::new(store.clone()),
        Arc::new(store.clone()),
        Arc::new(NoopCeremonyEventSubscriber),
    );
    // Every snapshot boundary, including directly before and after the reset,
    // must reconstruct the exact same full history and current coordinate.
    for cut in 1..=records.len() {
        store.forget(&id()).await.unwrap();
        let prefix = SessionStream::fold_records(&records[..cut]).unwrap();
        store
            .save(CeremonySnapshot {
                version: StreamVersion::new(cut as u64),
                instance: prefix.instance,
            })
            .await
            .unwrap();
        assert_eq!(
            stream.load(&id()).await.unwrap().instance,
            expected,
            "snapshot cut {cut}"
        );
    }
    store.forget(&id()).await.unwrap();
    assert_eq!(stream.load(&id()).await.unwrap().instance, expected);
    assert_eq!(
        records
            .iter()
            .filter(|r| matches!(r.event(), Some(CeremonyEvent::TransitionApplied(_))))
            .count(),
        3
    );
}

#[tokio::test]
async fn state_visit_child_process() {
    let Some(path) = std::env::var_os(PROCESS_PATH) else {
        return;
    };
    let phase = std::env::var(PROCESS_PHASE).unwrap();
    let engine = EmbeddedMade::open(path).unwrap();
    let mounted = engine.mount_yaml(DEFINITION).await.unwrap();
    let definition = &mounted.definitions()[0];
    if phase == "prefix" {
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
        work(&engine, "a", 1, 1, false).await;
        assert!(engine
            .instance(&id())
            .await
            .unwrap()
            .transitions()
            .is_empty());
        work(&engine, "a", 1, 2, true).await;
        transition(&engine, "next").await;
        work(&engine, "b", 2, 1, true).await;
    } else {
        let reopened = engine.instance(&id()).await.unwrap();
        assert_eq!(reopened.current_state_visit().get(), 2);
        assert_eq!(
            reopened
                .step_record(&step("a"))
                .unwrap()
                .state_iteration()
                .get(),
            2
        );
        transition(&engine, "back").await;
        let reset = engine.instance(&id()).await.unwrap();
        assert_eq!(reset.current_state_visit().get(), 3);
        assert_eq!(reset.current_state_iteration().get(), 1);
        assert_eq!(
            reset.step_record(&step("a")).unwrap().status(),
            StepStatus::Pending
        );
        assert_eq!(reset.step_record_history(&step("a")).len(), 2);
        work(&engine, "a", 3, 1, false).await;
        work(&engine, "a", 3, 2, true).await;
        let before = engine.audit_records(&id()).await.unwrap();
        assert!(engine
            .apply_transition(ApplyCeremonyTransitionInput::new(
                id(),
                role(),
                AuditActorKind::Agent,
                TransitionTrigger::new("next").unwrap()
            ))
            .await
            .is_err());
        assert_eq!(engine.audit_records(&id()).await.unwrap(), before);
        transition(&engine, "finish").await;
        assert_final(&engine.instance(&id()).await.unwrap());
    }
}

async fn work(engine: &EmbeddedMade, name: &str, visit: u32, iteration: u32, ready: bool) {
    let claimed = engine
        .start_step(StartCeremonyStepInput::new(
            id(),
            role(),
            AuditActorKind::Agent,
            step(name),
            LeaseOwnerId::new("host").unwrap(),
            IdempotencyKey::new(format!("{visit}-{iteration}-{name}")).unwrap(),
            DurationMs::from_millis(60_000),
        ))
        .await
        .unwrap();
    let record = claimed.instance().step_record(&step(name)).unwrap();
    assert_eq!(
        (
            record.state_visit().get(),
            record.state_iteration().get(),
            record.iteration().get(),
            record.attempt().get()
        ),
        (visit, iteration, 1, 1)
    );
    let output = StepOutput::new(
        Attributes::new(BTreeMap::from([
            ("ready".to_owned(), json!(ready)),
            ("marker".to_owned(), json!(format!("{visit}-{iteration}"))),
        ]))
        .unwrap(),
    );
    engine
        .complete_step(CompleteCeremonyStepInput::new(
            id(),
            step(name),
            StepResult::completed(output).unwrap(),
            AuditActorKind::Agent,
            claimed.claim_fence().clone(),
        ))
        .await
        .unwrap();
}

async fn transition(engine: &EmbeddedMade, trigger: &str) {
    engine
        .apply_transition(ApplyCeremonyTransitionInput::new(
            id(),
            role(),
            AuditActorKind::Agent,
            TransitionTrigger::new(trigger).unwrap(),
        ))
        .await
        .unwrap();
}

fn assert_final(instance: &CeremonyInstance) {
    assert_eq!(instance.current_state_visit().get(), 4);
    assert_eq!(instance.current_state().as_str(), "DONE");
    assert_eq!(instance.transitions().len(), 3);
    let a = step("a");
    assert_eq!(
        instance
            .step_record_history(&a)
            .iter()
            .map(|r| (r.state_visit().get(), r.state_iteration().get()))
            .collect::<Vec<_>>(),
        [(1, 1), (1, 2), (3, 1)]
    );
    let current = instance.step_record(&a).unwrap();
    assert_eq!(
        (current.state_visit().get(), current.state_iteration().get()),
        (3, 2)
    );
}
