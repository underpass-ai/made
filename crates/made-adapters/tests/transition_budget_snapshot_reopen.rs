#![cfg(feature = "sqlite")]

use std::collections::BTreeSet;
use std::sync::Arc;

use made_adapters::sqlite::SqliteCeremonyStore;
use made_app::services::SessionStream;
use made_core::entities::ceremony_commands::ApplyTransition;
use made_core::entities::ceremony_events::{CeremonyInstanceStarted, TransitionApplied};
use made_core::entities::{AuditFact, CeremonyCommand, CeremonyDefinition, CeremonyEvent};
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyEventStorePort, CeremonySnapshot, CeremonySnapshotStorePort,
    NoopCeremonyEventSubscriber,
};
use made_core::value_objects::{
    AuditActor, AuditActorKind, CeremonyContext, CeremonyId, CeremonyName, CeremonyRole,
    CeremonyState, CeremonyTransition, CeremonyTransitionRecord, CeremonyVersion, EventId,
    MaxBounces, RoleAction, RoleId, StateId, StateIteration, StreamVersion, TransitionTrigger,
};
use tempfile::TempDir;
use time::{Duration, OffsetDateTime};

#[tokio::test]
async fn snapshot_tail_and_sqlite_reopen_keep_transition_budget_counts() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("transition-budget.sqlite3");
    let id = CeremonyId::new("transition-budget-reopen").unwrap();
    let definition = definition();
    let events = events(&id, definition.name());

    {
        let store = SqliteCeremonyStore::open(&path).unwrap();
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
            .save(CeremonySnapshot {
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

    let reopened = SqliteCeremonyStore::open(&path).unwrap();
    let session = SessionStream::new(
        Arc::new(reopened.clone()),
        Arc::new(reopened),
        Arc::new(NoopCeremonyEventSubscriber),
    )
    .load(&id)
    .await
    .unwrap();
    assert_eq!(session.instance.transitions().len(), 2);
    let refusal = session
        .instance
        .decide(
            &CeremonyCommand::ApplyTransition(ApplyTransition {
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
            reason: "ceremony transition bounce limit exhausted"
        }
    ));
}

fn definition() -> CeremonyDefinition {
    CeremonyDefinition::new_with_transition_budgets(
        CeremonyName::new("transition_budget_reopen").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(state("A")),
            CeremonyState::intermediate(state("B")),
        ],
        vec![
            CeremonyTransition::new(state("A"), state("B"), trigger("next"), Vec::new()).unwrap(),
            CeremonyTransition::new(state("B"), state("A"), trigger("back"), Vec::new()).unwrap(),
        ],
        Vec::new(),
        Vec::new(),
        vec![CeremonyRole::new(
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

fn events(id: &CeremonyId, name: &CeremonyName) -> Vec<CeremonyEvent> {
    let driver = Some(RoleId::new("DRIVER").unwrap());
    vec![
        CeremonyEvent::CeremonyInstanceStarted(CeremonyInstanceStarted {
            ceremony_id: id.clone(),
            definition_name: name.clone(),
            definition_version: CeremonyVersion::v1(),
            initial_state: state("A"),
            step_ids: BTreeSet::new(),
            context: CeremonyContext::empty(),
            bound_definition: None,
            created_at: at(0),
        }),
        CeremonyEvent::TransitionApplied(TransitionApplied {
            destination: None,
            transition: CeremonyTransitionRecord::record_at(
                trigger("next"),
                state("A"),
                StateIteration::FIRST,
                state("B"),
                driver.clone(),
                at(1),
            ),
        }),
        CeremonyEvent::TransitionApplied(TransitionApplied {
            destination: None,
            transition: CeremonyTransitionRecord::record_at(
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
    id: &CeremonyId,
    name: &CeremonyName,
    events: &[CeremonyEvent],
    ordinal: usize,
) -> Vec<AuditFact> {
    events
        .iter()
        .cloned()
        .enumerate()
        .map(|(offset, event)| AuditFact {
            event_id: EventId::new(format!("transition-budget-{}", ordinal + offset)).unwrap(),
            event,
            ceremony_id: id.clone(),
            definition_name: name.clone(),
            definition_version: CeremonyVersion::v1(),
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
