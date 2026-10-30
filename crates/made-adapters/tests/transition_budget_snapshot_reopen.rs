#![cfg(feature = "sqlite")]

use std::collections::BTreeSet;
use std::sync::Arc;

use made_adapters::sqlite::SqliteProtocolStore;
use made_app::services::SessionStream;
use made_core::entities::protocol_commands::ApplyTransition;
use made_core::entities::protocol_events::{ProtocolInstanceStarted, TransitionApplied};
use made_core::entities::{AuditFact, ProtocolCommand, ProtocolDefinition, ProtocolEvent};
use made_core::error::DomainError;
use made_core::ports::{
    NoopProtocolEventSubscriber, ProtocolEventStorePort, ProtocolSnapshot,
    ProtocolSnapshotStorePort,
};
use made_core::value_objects::{
    AuditActor, AuditActorKind, EventId, MaxBounces, ProtocolContext, ProtocolId, ProtocolName,
    ProtocolRole, ProtocolState, ProtocolTransition, ProtocolTransitionRecord, ProtocolVersion,
    RoleAction, RoleId, StateId, StateIteration, StreamVersion, TransitionTrigger,
};
use tempfile::TempDir;
use time::{Duration, OffsetDateTime};

#[tokio::test]
async fn snapshot_tail_and_sqlite_reopen_keep_transition_budget_counts() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("transition-budget.sqlite3");
    let id = ProtocolId::new("transition-budget-reopen").unwrap();
    let definition = definition();
    let events = events(&id, definition.name());

    {
        let store = SqliteProtocolStore::open(&path).unwrap();
        let prefix = store
            .append(
                &id,
                StreamVersion::EMPTY,
                facts(&id, definition.name(), &events[..2], 1),
            )
            .await
            .unwrap();
        let folded = SessionStream::fold_records(prefix.records()).unwrap();
        store
            .save(ProtocolSnapshot {
                version: folded.version,
                instance: folded.instance,
            })
            .await
            .unwrap();
        store
            .append(
                &id,
                StreamVersion::new(2),
                facts(&id, definition.name(), &events[2..], 3),
            )
            .await
            .unwrap();
    }

    let reopened = SqliteProtocolStore::open(&path).unwrap();
    let session = SessionStream::new(
        Arc::new(reopened.clone()),
        Arc::new(reopened),
        Arc::new(NoopProtocolEventSubscriber),
    )
    .load(&id)
    .await
    .unwrap();
    assert_eq!(session.instance.transitions().len(), 2);
    let refusal = session
        .instance
        .decide(
            &ProtocolCommand::ApplyTransition(ApplyTransition {
                role_id: Some(RoleId::new("DRIVER").unwrap()),
                trigger: trigger("next"),
                now: at(3),
            }),
            &definition,
        )
        .unwrap_err();
    assert!(matches!(
        refusal,
        DomainError::InvariantViolated {
            reason: "protocol transition bounce limit exhausted"
        }
    ));
}

fn definition() -> ProtocolDefinition {
    ProtocolDefinition::new_with_transition_budgets(
        ProtocolName::new("transition_budget_reopen").unwrap(),
        ProtocolVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            ProtocolState::initial(state("A")),
            ProtocolState::intermediate(state("B")),
        ],
        vec![
            ProtocolTransition::new(state("A"), state("B"), trigger("next"), Vec::new()).unwrap(),
            ProtocolTransition::new(state("B"), state("A"), trigger("back"), Vec::new()).unwrap(),
        ],
        Vec::new(),
        Vec::new(),
        vec![ProtocolRole::new(
            RoleId::new("DRIVER").unwrap(),
            vec![
                RoleAction::transition(trigger("next")),
                RoleAction::transition(trigger("back")),
            ],
        )
        .unwrap()],
        None,
        Some(MaxBounces::new(1).unwrap()),
    )
    .unwrap()
}

fn events(id: &ProtocolId, name: &ProtocolName) -> Vec<ProtocolEvent> {
    let driver = Some(RoleId::new("DRIVER").unwrap());
    vec![
        ProtocolEvent::ProtocolInstanceStarted(ProtocolInstanceStarted {
            protocol_id: id.clone(),
            definition_name: name.clone(),
            definition_version: ProtocolVersion::v1(),
            initial_state: state("A"),
            step_ids: BTreeSet::new(),
            context: ProtocolContext::empty(),
            bound_definition: None,
            lineage: None,
            succession: None,
            budget_account_id: None,
            protocol_deadline: None,
            state_deadline: None,
            created_at: at(0),
        }),
        ProtocolEvent::TransitionApplied(TransitionApplied {
            destination: None,
            transition: ProtocolTransitionRecord::record_at(
                trigger("next"),
                state("A"),
                StateIteration::FIRST,
                state("B"),
                driver.clone(),
                at(1),
            ),
        }),
        ProtocolEvent::TransitionApplied(TransitionApplied {
            destination: None,
            transition: ProtocolTransitionRecord::record_at(
                trigger("back"),
                state("B"),
                StateIteration::FIRST,
                state("A"),
                driver,
                at(2),
            ),
        }),
    ]
}

fn facts(
    id: &ProtocolId,
    name: &ProtocolName,
    events: &[ProtocolEvent],
    ordinal: usize,
) -> Vec<AuditFact> {
    events
        .iter()
        .cloned()
        .enumerate()
        .map(|(offset, event)| AuditFact {
            event_id: EventId::new(format!("transition-budget-{}", ordinal + offset)).unwrap(),
            event,
            protocol_id: id.clone(),
            definition_name: name.clone(),
            definition_version: ProtocolVersion::v1(),
            occurred_at: at((ordinal + offset) as i64),
            actor: AuditActor::new("transition-budget-test", AuditActorKind::Engine, None).unwrap(),
            correlation_id: None,
            causation_id: None,
            trace: None,
        })
        .collect()
}

fn state(raw: &str) -> StateId {
    StateId::new(raw).unwrap()
}

fn trigger(raw: &str) -> TransitionTrigger {
    TransitionTrigger::new(raw).unwrap()
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}
