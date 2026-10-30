#![cfg(feature = "sqlite")]

use std::collections::BTreeSet;
use std::sync::Arc;

use made_adapters::sqlite::SqliteProtocolStore;
use made_app::services::{LoadedSession, SessionStream};
use made_core::entities::protocol_events::{
    ProtocolInstanceStarted, StateIterationStarted, StepCompleted, StepStarted,
};
use made_core::entities::{AuditFact, ProtocolEvent};
use made_core::ports::{
    NoopProtocolEventSubscriber, ProtocolEventStorePort, ProtocolSnapshot,
    ProtocolSnapshotStorePort,
};
use made_core::value_objects::{
    AuditActor, AuditActorKind, DurationMs, EventId, EventSchemaVersion, IdempotencyKey,
    LeaseOwnerId, ProtocolContext, ProtocolEventPageLimit, ProtocolId, ProtocolName,
    ProtocolVersion, RoleId, StateId, StateIteration, StepAttempt, StepId, StepIteration,
    StepLease, StepOutput, StepResult, StepStatus, StreamVersion,
};
use tempfile::TempDir;
use time::{Duration, OffsetDateTime};

const SNAPSHOT_VERSION: StreamVersion = StreamVersion::new(7);
const FINAL_VERSION: StreamVersion = StreamVersion::new(10);

#[tokio::test]
async fn state_and_step_iterations_survive_snapshot_tail_and_sqlite_reopen() {
    let directory = TempDir::new().expect("a temporary directory");
    let path = directory.path().join("state-repeat.sqlite3");
    let protocol_id = ProtocolId::new("state-repeat-snapshot-reopen").unwrap();
    let definition_name = ProtocolName::new("state_repeat_snapshot_reopen").unwrap();
    let step_id = StepId::new("repeatable_work").unwrap();
    let events = repeated_state_events(&protocol_id, &definition_name, &step_id);

    let full_fold = {
        let store = SqliteProtocolStore::open(&path).expect("the store opens");
        let prefix = store
            .append(
                &protocol_id,
                StreamVersion::EMPTY,
                facts(&protocol_id, &definition_name, &events[..7], 1),
            )
            .await
            .expect("the prefix appends");
        assert_eq!(prefix.appended_version(), Some(SNAPSHOT_VERSION));

        let snapshot_fold = SessionStream::fold_records(prefix.records()).expect("prefix folds");
        assert_snapshot_coordinates(&snapshot_fold, &step_id);
        store
            .save(ProtocolSnapshot {
                version: snapshot_fold.version,
                instance: snapshot_fold.instance,
            })
            .await
            .expect("the midpoint snapshot is durable");

        let tail = store
            .append(
                &protocol_id,
                SNAPSHOT_VERSION,
                facts(&protocol_id, &definition_name, &events[7..], 8),
            )
            .await
            .expect("the tail appends");
        assert_eq!(tail.appended_version(), Some(FINAL_VERSION));

        let records = store
            .read(
                &protocol_id,
                StreamVersion::EMPTY,
                ProtocolEventPageLimit::DEFAULT,
            )
            .await
            .expect("the complete stream reads");
        assert_eq!(records.len(), FINAL_VERSION.value() as usize);
        assert!(records
            .iter()
            .filter_map(|record| record.event())
            .all(|event| !matches!(
                event,
                ProtocolEvent::StepStarted(_) | ProtocolEvent::StepCompleted(_)
            ) || event.schema_version() == EventSchemaVersion::V2));

        let full_fold = SessionStream::fold_records(&records).expect("the complete stream folds");
        let snapshot_tail = stream(store.clone())
            .load(&protocol_id)
            .await
            .expect("the midpoint snapshot and tail load");
        assert_same_fold(&full_fold, &snapshot_tail, &step_id);
        full_fold
    };

    let reopened = SqliteProtocolStore::open(&path).expect("the store reopens");
    let after_reopen = stream(reopened.clone())
        .load(&protocol_id)
        .await
        .expect("the reopened store loads from its snapshot and tail");
    assert_same_fold(&full_fold, &after_reopen, &step_id);

    reopened
        .forget(&protocol_id)
        .await
        .expect("snapshot deletion succeeds");
    let without_snapshot = stream(reopened)
        .load(&protocol_id)
        .await
        .expect("the stream folds after snapshot deletion");
    assert_same_fold(&full_fold, &without_snapshot, &step_id);
}

fn stream(store: SqliteProtocolStore) -> SessionStream {
    SessionStream::new(
        Arc::new(store.clone()),
        Arc::new(store),
        Arc::new(NoopProtocolEventSubscriber),
    )
}

fn assert_snapshot_coordinates(snapshot: &LoadedSession, step_id: &StepId) {
    assert_eq!(snapshot.version, SNAPSHOT_VERSION);
    assert_eq!(snapshot.instance.current_state_iteration().get(), 2);
    assert_eq!(
        coordinates(snapshot, step_id),
        (2, 1, StepStatus::InProgress)
    );
    assert_eq!(history_coordinates(snapshot, step_id), [(1, 1), (1, 2)]);
}

fn assert_same_fold(expected: &LoadedSession, actual: &LoadedSession, step_id: &StepId) {
    assert_eq!(actual.version, FINAL_VERSION);
    assert_eq!(actual.version, expected.version);
    assert_eq!(actual.head_event_id(), expected.head_event_id());
    assert_eq!(actual.instance, expected.instance);
    assert_eq!(actual.instance.current_state_iteration().get(), 2);
    assert_eq!(coordinates(actual, step_id), (2, 2, StepStatus::Completed));
    assert_eq!(
        history_coordinates(actual, step_id),
        [(1, 1), (1, 2), (2, 1)]
    );
}

fn coordinates(session: &LoadedSession, step_id: &StepId) -> (u32, u32, StepStatus) {
    let record = session
        .instance
        .step_record(step_id)
        .expect("the current step record exists");
    (
        record.state_iteration().get(),
        record.iteration().get(),
        record.status(),
    )
}

fn history_coordinates(session: &LoadedSession, step_id: &StepId) -> Vec<(u32, u32)> {
    session
        .instance
        .step_record_history(step_id)
        .iter()
        .map(|record| {
            assert_eq!(record.status(), StepStatus::Completed);
            (record.state_iteration().get(), record.iteration().get())
        })
        .collect()
}

fn repeated_state_events(
    protocol_id: &ProtocolId,
    definition_name: &ProtocolName,
    step_id: &StepId,
) -> Vec<ProtocolEvent> {
    let state_id = StateId::new("WORKING").unwrap();
    let role_id = RoleId::new("WORKER").unwrap();
    let second_state_iteration = StateIteration::new(2).unwrap();
    let second_step_iteration = StepIteration::new(2).unwrap();

    vec![
        ProtocolEvent::ProtocolInstanceStarted(ProtocolInstanceStarted {
            protocol_id: protocol_id.clone(),
            definition_name: definition_name.clone(),
            definition_version: ProtocolVersion::v1(),
            initial_state: state_id.clone(),
            step_ids: BTreeSet::from([step_id.clone()]),
            context: ProtocolContext::empty(),
            bound_definition: None,
            lineage: None,
            succession: None,
            budget_account_id: None,
            protocol_deadline: None,
            state_deadline: None,
            created_at: at(0),
        }),
        started(
            step_id,
            StateIteration::FIRST,
            StepIteration::FIRST,
            &role_id,
            1,
        ),
        completed(
            step_id,
            StateIteration::FIRST,
            StepIteration::FIRST,
            Some(second_step_iteration),
            &role_id,
            2,
        ),
        started(
            step_id,
            StateIteration::FIRST,
            second_step_iteration,
            &role_id,
            3,
        ),
        completed(
            step_id,
            StateIteration::FIRST,
            second_step_iteration,
            None,
            &role_id,
            4,
        ),
        ProtocolEvent::StateIterationStarted(StateIterationStarted {
            state_visit: None,
            state_id,
            state_iteration: second_state_iteration,
            step_ids: vec![step_id.clone()],
            started_at: at(5),
        }),
        started(
            step_id,
            second_state_iteration,
            StepIteration::FIRST,
            &role_id,
            6,
        ),
        completed(
            step_id,
            second_state_iteration,
            StepIteration::FIRST,
            Some(second_step_iteration),
            &role_id,
            7,
        ),
        started(
            step_id,
            second_state_iteration,
            second_step_iteration,
            &role_id,
            8,
        ),
        completed(
            step_id,
            second_state_iteration,
            second_step_iteration,
            None,
            &role_id,
            9,
        ),
    ]
}

fn started(
    step_id: &StepId,
    state_iteration: StateIteration,
    iteration: StepIteration,
    role_id: &RoleId,
    ordinal: i64,
) -> ProtocolEvent {
    ProtocolEvent::StepStarted(StepStarted {
        state_visit: None,
        step_id: step_id.clone(),
        state_iteration: Some(state_iteration),
        iteration,
        attempt: StepAttempt::FIRST,
        lease: StepLease::acquire(
            LeaseOwnerId::new(format!("worker-{ordinal}")).unwrap(),
            IdempotencyKey::new(format!("state-repeat-{ordinal}")).unwrap(),
            at(ordinal),
            DurationMs::from_millis(60_000),
        )
        .unwrap(),
        started_by: role_id.clone(),
        role_from: None,
        sealed_role: None,
        budget_reservation_id: None,
        deadline: None,
        started_at: at(ordinal),
    })
}

fn completed(
    step_id: &StepId,
    state_iteration: StateIteration,
    iteration: StepIteration,
    next_iteration: Option<StepIteration>,
    role_id: &RoleId,
    ordinal: i64,
) -> ProtocolEvent {
    ProtocolEvent::StepCompleted(StepCompleted {
        state_visit: None,
        step_id: step_id.clone(),
        state_iteration: Some(state_iteration),
        iteration,
        attempt: StepAttempt::FIRST,
        result: StepResult::completed(StepOutput::empty()).unwrap(),
        next_iteration,
        finished_by: role_id.clone(),
        finished_at: at(ordinal),
    })
}

fn facts(
    protocol_id: &ProtocolId,
    definition_name: &ProtocolName,
    events: &[ProtocolEvent],
    first_ordinal: usize,
) -> Vec<AuditFact> {
    events
        .iter()
        .cloned()
        .enumerate()
        .map(|(offset, event)| AuditFact {
            event_id: EventId::new(format!("state-repeat-{:02}", first_ordinal + offset)).unwrap(),
            event,
            protocol_id: protocol_id.clone(),
            definition_name: definition_name.clone(),
            definition_version: ProtocolVersion::v1(),
            occurred_at: at((first_ordinal + offset) as i64),
            actor: AuditActor::new("state-repeat-test", AuditActorKind::Engine, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        })
        .collect()
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}
