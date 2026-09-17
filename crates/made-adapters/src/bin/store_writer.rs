//! A second process for the two-hosts test.
//!
//! Appends `count` events to the ceremony's stream at `path`, printing each
//! acknowledged version so the parent can tell what was acknowledged before
//! it decides what must survive.
//!
//! Its own binary because the property under test is *processes*, not tasks:
//! two threads sharing one handle would prove nothing about a store that two
//! agent hosts open independently.
//!
//! Usage: store_writer <path> <ceremony-id> <count>

use std::io::Write;

use made_adapters::sqlite::SqliteCeremonyStore;
use made_core::entities::ceremony_events::StepCompleted;
use made_core::entities::CeremonyEvent;
use made_core::entities::{AuditFact, CeremonyDefinition};
use made_core::ports::{AppendOutcome, CeremonyEventStorePort};
use made_core::value_objects::{
    AuditActor, AuditActorKind, CeremonyId, CeremonyName, CeremonyState, CeremonyTransition,
    CeremonyVersion, EventId, StateId, TransitionTrigger,
};
use made_core::value_objects::{
    RoleId, StepAttempt, StepId, StepIteration, StepOutput, StepResult,
};
use time::OffsetDateTime;

fn definition() -> CeremonyDefinition {
    CeremonyDefinition::new(
        CeremonyName::new("shared_store_ceremony").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(StateId::new("OPEN").unwrap()),
            CeremonyState::terminal(StateId::new("DONE").unwrap()),
        ],
        vec![CeremonyTransition::new(
            StateId::new("OPEN").unwrap(),
            StateId::new("DONE").unwrap(),
            TransitionTrigger::new("finish").unwrap(),
            Vec::new(),
        )
        .unwrap()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap()
}

/// How many times an append is re-decided after a conflict before the
/// writer gives up. Each writer owns its stream, so one attempt should
/// do; the bound is there so a store that conflicts forever fails the
/// test instead of hanging it.
const APPEND_ATTEMPTS: u32 = 16;

fn fact_for(ceremony_id: &CeremonyId, ordinal: u64) -> AuditFact {
    let definition = definition();
    AuditFact {
        event_id: EventId::new(format!("{}-{ordinal}", ceremony_id.as_str())).unwrap(),
        event: CeremonyEvent::StepCompleted(StepCompleted {
            step_id: StepId::new("conformance_step").unwrap(),
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            result: StepResult::completed(StepOutput::empty()).unwrap(),
            next_iteration: None,
            finished_by: RoleId::new("writer").unwrap(),
            finished_at: OffsetDateTime::UNIX_EPOCH,
        }),
        ceremony_id: ceremony_id.clone(),
        definition_name: definition.name().clone(),
        definition_version: definition.version().clone(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("writer", AuditActorKind::Engine, None).unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: store_writer <path> <ceremony-id> <count>");
    let ceremony_id = CeremonyId::new(args.next().expect("ceremony id")).expect("valid id");
    let count: u64 = args.next().expect("count").parse().expect("count");

    let store = match SqliteCeremonyStore::open(&path) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("store_writer: could not open the store: {error}");
            std::process::exit(1);
        }
    };

    write_events(&store, &ceremony_id, count).await;
}

/// Reload-on-conflict, the way a use case does: read the head, decide
/// against it, append with it as the expectation, and start over when
/// somebody else moved the stream first.
async fn write_events(store: &SqliteCeremonyStore, ceremony_id: &CeremonyId, count: u64) {
    let stdout = std::io::stdout();
    for ordinal in 1..=count {
        let mut landed = None;
        for _ in 0..APPEND_ATTEMPTS {
            let expected = store.head(ceremony_id).await.expect("head should read");
            let outcome = store
                .append(ceremony_id, expected, vec![fact_for(ceremony_id, ordinal)])
                .await
                .expect("append should succeed or conflict, not fail");
            match outcome {
                AppendOutcome::Appended { version, .. } => {
                    landed = Some(version);
                    break;
                }
                AppendOutcome::Conflict { .. } => {}
            }
        }
        let Some(version) = landed else {
            eprintln!(
                "store_writer: event {ordinal} of {} conflicted {APPEND_ATTEMPTS} times",
                ceremony_id.as_str()
            );
            std::process::exit(1);
        };
        let mut lock = stdout.lock();
        writeln!(lock, "{}", version.value()).expect("stdout write");
        lock.flush().expect("stdout flush");
    }
}
