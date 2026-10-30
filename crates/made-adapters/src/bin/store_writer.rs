//! A second process for the two-hosts test.
//!
//! Appends `count` events to the protocol's stream at `path`, printing each
//! acknowledged version so the parent can tell what was acknowledged before
//! it decides what must survive.
//!
//! Its own binary because the property under test is *processes*, not tasks:
//! two threads sharing one handle would prove nothing about a store that two
//! agent hosts open independently.
//!
//! Usage: store_writer <path> <protocol-id> <count>

use std::io::Write;

use made_adapters::sqlite::SqliteProtocolStore;
use made_core::entities::protocol_events::StepCompleted;
use made_core::entities::ProtocolEvent;
use made_core::entities::{AuditFact, ProtocolDefinition};
use made_core::ports::{AppendOutcome, ProtocolEventStorePort};
use made_core::value_objects::{
    AuditActor, AuditActorKind, EventId, ProtocolId, ProtocolName, ProtocolState,
    ProtocolTransition, ProtocolVersion, StateId, TransitionTrigger,
};
use made_core::value_objects::{
    RoleId, StepAttempt, StepId, StepIteration, StepOutput, StepResult,
};
use time::OffsetDateTime;

fn definition() -> ProtocolDefinition {
    ProtocolDefinition::new(
        ProtocolName::new("shared_store_protocol").unwrap(),
        ProtocolVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            ProtocolState::initial(StateId::new("OPEN").unwrap()),
            ProtocolState::terminal(StateId::new("DONE").unwrap()),
        ],
        vec![ProtocolTransition::new(
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

fn fact_for(protocol_id: &ProtocolId, ordinal: u64) -> AuditFact {
    let definition = definition();
    AuditFact {
        event_id: EventId::new(format!("{}-{ordinal}", protocol_id.as_str())).unwrap(),
        event: ProtocolEvent::StepCompleted(StepCompleted {
            state_visit: None,
            step_id: StepId::new("conformance_step").unwrap(),
            state_iteration: None,
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            result: StepResult::completed(StepOutput::empty()).unwrap(),
            next_iteration: None,
            finished_by: RoleId::new("writer").unwrap(),
            finished_at: OffsetDateTime::UNIX_EPOCH,
        }),
        protocol_id: protocol_id.clone(),
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
        .expect("usage: store_writer <path> <protocol-id> <count>");
    let protocol_id = ProtocolId::new(args.next().expect("protocol id")).expect("valid id");
    let count: u64 = args.next().expect("count").parse().expect("count");

    let store = match SqliteProtocolStore::open(&path) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("store_writer: could not open the store: {error}");
            std::process::exit(1);
        }
    };

    write_events(&store, &protocol_id, count).await;
}

/// Reload-on-conflict, the way a use case does: read the head, decide
/// against it, append with it as the expectation, and start over when
/// somebody else moved the stream first.
async fn write_events(store: &SqliteProtocolStore, protocol_id: &ProtocolId, count: u64) {
    let stdout = std::io::stdout();
    for ordinal in 1..=count {
        let mut landed = None;
        for _ in 0..APPEND_ATTEMPTS {
            let expected = store.head(protocol_id).await.expect("head should read");
            let outcome = store
                .append(protocol_id, expected, vec![fact_for(protocol_id, ordinal)])
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
                protocol_id.as_str()
            );
            std::process::exit(1);
        };
        let mut lock = stdout.lock();
        writeln!(lock, "{}", version.value()).expect("stdout write");
        lock.flush().expect("stdout flush");
    }
}
