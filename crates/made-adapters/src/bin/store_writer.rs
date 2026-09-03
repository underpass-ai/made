//! A second process for the two-hosts test.
//!
//! Commits `count` ceremony revisions to the store at `path`, printing each
//! committed revision so the parent can tell what was acknowledged before it
//! decides what must survive.
//!
//! Its own binary because the property under test is *processes*, not tasks:
//! two threads sharing one handle would prove nothing about a store that two
//! agent hosts open independently.
//!
//! Usage: store_writer <path> <ceremony-id> <count>

use std::io::Write;

use made_adapters::sqlite::SqliteCeremonyStore;
use made_core::entities::{AuditFact, CeremonyCommit, CeremonyDefinition, CeremonyInstance};
use made_core::ports::CeremonyUnitOfWorkPort;
use made_core::value_objects::{
    AuditActor, AuditActorKind, AuditEventType, CeremonyContext, CeremonyId, CeremonyName,
    CeremonyState, CeremonyTransition, CeremonyVersion, EventId, ExpectedRevision, StateId,
    TransitionTrigger,
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

fn commit_for(
    ceremony_id: &CeremonyId,
    expected: ExpectedRevision,
    ordinal: u64,
) -> CeremonyCommit {
    let definition = definition();
    let instance = CeremonyInstance::start(
        ceremony_id.clone(),
        &definition,
        CeremonyContext::empty(),
        OffsetDateTime::UNIX_EPOCH,
    );
    let fact = AuditFact {
        event_id: EventId::new(format!("{}-{ordinal}", ceremony_id.as_str())).unwrap(),
        event_type: AuditEventType::StepCompleted,
        ceremony_id: ceremony_id.clone(),
        definition_name: definition.name().clone(),
        definition_version: definition.version().clone(),
        occurred_at: OffsetDateTime::UNIX_EPOCH,
        actor: AuditActor::new("writer", AuditActorKind::Engine, None).unwrap(),
        correlation_id: None,
        causation_id: None,
        trace: None,
    };
    CeremonyCommit::new(instance, expected, [fact], []).unwrap()
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

    let stdout = std::io::stdout();
    let mut expected = ExpectedRevision::New;
    for ordinal in 1..=count {
        let outcome = store
            .commit(commit_for(&ceremony_id, expected, ordinal))
            .await
            .expect("commit should succeed");
        let revision = outcome
            .committed_revision()
            .expect("a commit with a fresh expectation is not a conflict");
        expected = ExpectedRevision::Exactly(revision);
        let mut lock = stdout.lock();
        writeln!(lock, "{}", revision.value()).expect("stdout write");
        lock.flush().expect("stdout flush");
    }
}
