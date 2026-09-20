//! Intention to evidence, on the store the local edition ships with.
//!
//! One system, taken the whole way: written down, refused four
//! different ways, protected from a concurrent edit, sealed, run,
//! driven with real step evidence, advanced, read back, drawn — and
//! then found again after the process that wrote it is gone.
//!
//! Every refusal here is one somebody will actually hit. They are
//! checked together because they share a design: a test that proved
//! each on its own minimal fixture would prove nine things about nine
//! systems and nothing about one.

use made_adapters::yaml::CeremonyDefinitionYaml;
use made_app::usecases::agentic_system::{
    AgenticSystemDesignDocument, InstantiateAgenticSystemInput, ParticipantOffer,
};
use made_app::usecases::{ApplyCeremonyTransitionInput, CompleteCeremonyStepInput};
use made_core::ports::AgenticSystemQuery;
use made_core::value_objects::{
    AgenticSystemExecutionId, AgenticSystemId, AgenticSystemLifecycle, AgenticSystemPageLimit,
    AgenticSystemRevision, AuditActorId, AuditActorKind, Capability, CeremonyContext, CeremonyId,
    DurationMs, IdempotencyKey, LeaseOwnerId, LinkStatus, RoleId, Specialty, StepId, StepOutput,
    StepResult, SystemCeremonyId, TransitionTrigger,
};
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};
use tempfile::TempDir;

const DRAFTING: &str = r#"
version: "1.0"
name: system_drafting
states:
  - {id: OPEN, initial: true}
  - {id: DONE, terminal: true}
transitions:
  - {from: OPEN, to: DONE, trigger: finish}
steps:
  - {id: draft, state: OPEN, handler: host_callback}
roles:
  - {id: AUTHOR, allowed_actions: [draft, finish]}
  - {id: EDITOR, allowed_actions: [finish]}
retry_policies: {default: {max_attempts: 3, backoff_seconds: 0}}
"#;

const REVIEW: &str = r#"
version: "1.0"
name: system_review
states:
  - {id: OPEN, initial: true}
  - {id: DONE, terminal: true}
transitions:
  - {from: OPEN, to: DONE, trigger: finish}
steps:
  - {id: read_it, state: OPEN, handler: host_callback}
roles:
  - {id: REVIEWER, allowed_actions: [read_it, finish]}
  - {id: AUTHOR, allowed_actions: [finish]}
retry_policies: {default: {max_attempts: 3, backoff_seconds: 0}}
"#;

const SYSTEM_ID: &str = "delivery-system";
const RUN_ID: &str = "delivery-run-1";

#[tokio::test]
async fn a_system_goes_from_intention_to_evidence_and_survives_the_process() {
    let directory = TempDir::new().expect("a temporary directory");
    let path = directory.path().join("made.sqlite3");

    {
        let engine = EmbeddedMade::open(&path).expect("the durable store opens");
        publish_ceremonies(&engine).await;

        intention_becomes_a_design(&engine).await;
        validation_refuses_four_different_mistakes(&engine).await;
        a_concurrent_edit_is_refused(&engine).await;
        publishing_twice_seals_once(&engine).await;
        let run = instantiating_twice_opens_one_run(&engine).await;
        real_evidence_completes_the_first_ceremony(&engine, &run).await;
        advancing_starts_what_is_now_ready(&engine).await;
        the_view_tells_intended_from_observed(&engine).await;
        the_diagram_is_text_and_words(&engine).await;
    }

    a_reopened_store_still_knows_all_of_it(&path).await;
}

async fn publish_ceremonies(engine: &EmbeddedMade) {
    for yaml in [DRAFTING, REVIEW] {
        let definition = CeremonyDefinitionYaml::parse_str(yaml).expect("a valid definition");
        engine
            .publish_definition(definition)
            .await
            .expect("publishing succeeds");
    }
}

/// Somebody wants a draft written and independently read. That is the
/// whole intention, and it becomes a design without anything being
/// resolved, checked or sealed.
async fn intention_becomes_a_design(engine: &EmbeddedMade) {
    let view = engine
        .design_agentic_system(document(design()))
        .await
        .expect("the design saves");

    assert_eq!(view.revision(), AgenticSystemRevision::INITIAL);
    assert_eq!(
        view.system().lifecycle(),
        AgenticSystemLifecycle::Draft,
        "a design is a draft until somebody seals it"
    );

    let page = engine
        .list_agentic_systems(&AgenticSystemQuery::new(
            Some(AgenticSystemLifecycle::Draft),
            AgenticSystemPageLimit::new(10).unwrap(),
            None,
        ))
        .await
        .expect("the catalogue lists");
    assert_eq!(page.systems().len(), 1);
}

/// Four mistakes, each located at the element it is about.
///
/// They are checked on one deliberately broken design rather than on
/// four, because an author makes several at once and has to be able
/// to see all of them before fixing any.
async fn validation_refuses_four_different_mistakes(engine: &EmbeddedMade) {
    let mut broken = design();
    // A pin whose digest is not the published one: somebody copied a
    // design after the ceremony was republished.
    broken["ceremonies"][1]["pin"]["digest"] = json!("aa".repeat(32));
    // A seat the definition declares, bound to nobody.
    broken["ceremonies"][0]["role_bindings"] = json!({"AUTHOR": "writer"});
    // A reviewer who is the author's own opinion under another name:
    // both sit at the review, and both are one voice.
    broken["participants"][2]["binding"]["independence_group"] = json!("writing-desk");
    // And a dependency that never returns.
    broken["ceremonies"][0]["depends_on"] = json!(["review"]);
    broken["ceremonies"][1]["activation"] = json!({ "kind": "after_dependencies" });
    broken["id"] = json!("broken-system");
    broken["expected_revision"] = Value::Null;

    engine
        .design_agentic_system(document(broken))
        .await
        .expect("a broken design is still writable");
    let report = engine
        .validate_agentic_system(&system_id("broken-system"), None)
        .await
        .expect("validation answers");
    let said = report
        .findings()
        .iter()
        .map(made_core::value_objects::AgenticSystemValidationFinding::explain)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(!report.is_publishable(), "{said}");
    assert!(
        said.contains("digest") && said.contains("pin"),
        "the changed pin is not reported: {said}"
    );
    assert!(
        said.contains("no participant is bound to seat `EDITOR`"),
        "the unfilled seat is not reported: {said}"
    );
    assert!(
        said.contains("independence group `writing-desk`"),
        "the reviewer who is the author is not reported: {said}"
    );
    assert!(
        said.contains("wait for each other"),
        "the unbounded cycle is not reported: {said}"
    );
}

/// Two people read revision 1 and both write. The second is refused
/// with the revision that is actually current, rather than quietly
/// replacing an edit they never saw.
async fn a_concurrent_edit_is_refused(engine: &EmbeddedMade) {
    let mut mine = design();
    mine["expected_revision"] = json!(1);
    mine["purpose"] = json!("what I changed it to");
    let mut theirs = design();
    theirs["expected_revision"] = json!(1);
    theirs["purpose"] = json!("what they changed it to");

    let won = engine
        .design_agentic_system(document(mine))
        .await
        .expect("the first edit lands");
    assert_eq!(won.revision().get(), 2);

    let lost = engine
        .design_agentic_system(document(theirs))
        .await
        .expect_err("the stale edit is refused");
    assert!(
        lost.to_string().contains("revision 2"),
        "the refusal must name the revision to rebase onto: {lost}"
    );

    let head = engine
        .get_agentic_system(&system_id(SYSTEM_ID), None)
        .await
        .expect("the head reads");
    assert_eq!(head.system().purpose().as_str(), "what I changed it to");
}

/// Sealing the same revision twice is a retry, not a second
/// publication, and the second answer says so.
async fn publishing_twice_seals_once(engine: &EmbeddedMade) {
    let sealed = engine
        .publish_agentic_system(
            &system_id(SYSTEM_ID),
            AgenticSystemRevision::new(2).unwrap(),
        )
        .await
        .expect("a valid design publishes");
    assert_eq!(sealed.outcome().as_str(), "published");
    assert!(sealed.validation().is_publishable());

    let again = engine
        .publish_agentic_system(
            &system_id(SYSTEM_ID),
            AgenticSystemRevision::new(2).unwrap(),
        )
        .await
        .expect("the retry is answered");
    assert_eq!(again.outcome().as_str(), "already_published");
    assert_eq!(again.digest(), sealed.digest());
}

/// A host that did not hear the first answer asks again, and gets the
/// run it already opened rather than a second one beside it.
async fn instantiating_twice_opens_one_run(engine: &EmbeddedMade) -> CeremonyId {
    let first = Box::pin(engine.instantiate_agentic_system(instantiation()))
        .await
        .expect("the run opens");
    let again = Box::pin(engine.instantiate_agentic_system(instantiation()))
        .await
        .expect("the retry is answered");

    assert_eq!(first.id(), again.id());
    let drafting = first
        .link(&ceremony("drafting"))
        .expect("the drafting link exists");
    assert_eq!(drafting.status(), LinkStatus::Started);

    // The reviewer was never offered, so the ceremony that needed one
    // is skipped with the reason. Nothing stands in for a missing
    // party.
    let review = first.link(&ceremony("review")).expect("the review link");
    assert_eq!(review.status(), LinkStatus::Pending);

    drafting
        .instance_id()
        .cloned()
        .expect("a started link names its instance")
}

/// The ceremony is driven by claiming and completing its step for
/// real, so what the run later reports is what happened rather than
/// what it was told.
async fn real_evidence_completes_the_first_ceremony(engine: &EmbeddedMade, instance: &CeremonyId) {
    let claim = engine
        .start_step(made_app::usecases::StartCeremonyStepInput::new(
            instance.clone(),
            RoleId::new("AUTHOR").unwrap(),
            AuditActorKind::Agent,
            StepId::new("draft").unwrap(),
            LeaseOwnerId::new("walkthrough-host").unwrap(),
            IdempotencyKey::new("walkthrough-claim").unwrap(),
            DurationMs::from_millis(60_000),
        ))
        .await
        .expect("the step is claimed");
    engine
        .complete_step(CompleteCeremonyStepInput::new(
            instance.clone(),
            StepId::new("draft").unwrap(),
            StepResult::completed(StepOutput::empty()).unwrap(),
            AuditActorKind::Agent,
            claim.claim_fence().clone(),
        ))
        .await
        .expect("the step completes");
    let ended = engine
        .apply_transition(ApplyCeremonyTransitionInput::new(
            instance.clone(),
            RoleId::new("AUTHOR").unwrap(),
            AuditActorKind::Agent,
            TransitionTrigger::new("finish").unwrap(),
        ))
        .await
        .expect("the ceremony finishes");

    assert!(
        ended.lifecycle().end_reason().is_some(),
        "the ceremony must actually have ended"
    );
}

/// What can start now is read back from the instance that just
/// finished, not from anything the run was told.
async fn advancing_starts_what_is_now_ready(engine: &EmbeddedMade) {
    let advanced = Box::pin(engine.advance_agentic_system_execution(
        &AgenticSystemExecutionId::new(RUN_ID).unwrap(),
        &AuditActorId::new("walkthrough-host"),
        AuditActorKind::Service,
    ))
    .await
    .expect("the run advances");

    let drafting = advanced.link(&ceremony("drafting")).expect("the link");
    assert_eq!(drafting.status(), LinkStatus::Completed);

    let review = advanced.link(&ceremony("review")).expect("the link");
    assert_eq!(
        review.status(),
        LinkStatus::Skipped,
        "no reviewer was offered, so the review is skipped rather than simulated"
    );
    let why = review
        .skipped_because()
        .expect("a skip carries its reason")
        .as_str()
        .to_owned();
    assert!(
        why.contains("critic") && why.contains("offered nobody"),
        "the reason must name who was missing: {why}"
    );
}

async fn the_view_tells_intended_from_observed(engine: &EmbeddedMade) {
    let view = engine
        .get_agentic_system_execution(&AgenticSystemExecutionId::new(RUN_ID).unwrap())
        .await
        .expect("the run reads back");

    let drafting = view
        .ceremonies()
        .iter()
        .find(|ceremony| ceremony.ceremony().as_str() == "drafting")
        .expect("the drafting view");
    assert_eq!(drafting.planned(), LinkStatus::Completed);
    assert!(
        drafting.observed_lifecycle().is_some(),
        "a link with an instance reports what that instance says"
    );

    let review = view
        .ceremonies()
        .iter()
        .find(|ceremony| ceremony.ceremony().as_str() == "review")
        .expect("the review view");
    assert_eq!(review.planned(), LinkStatus::Skipped);
    assert!(
        review.observed_lifecycle().is_none(),
        "a ceremony nothing opened has nothing to observe, and says nothing rather than guessing"
    );
    assert_eq!(
        view.system().revision().get(),
        2,
        "a run is explained by the revision it pinned, not by the head"
    );
}

async fn the_diagram_is_text_and_words(engine: &EmbeddedMade) {
    let diagram = engine
        .render_agentic_system_diagram(
            &system_id(SYSTEM_ID),
            None,
            Some(&AgenticSystemExecutionId::new(RUN_ID).unwrap()),
        )
        .await
        .expect("the diagram renders");

    assert!(diagram.mermaid().starts_with("flowchart LR"));
    assert!(diagram.mermaid().contains("class c_review linkSkipped"));
    let words = diagram.text_equivalent().join("\n");
    assert!(words.contains("Observed: skipped."));
    assert!(
        words.contains("Edge styles:"),
        "the legend belongs with the picture, in both forms"
    );
}

/// The process that wrote all of it is gone. The design, the seal and
/// the run are still there, and the run still names the revision it
/// pinned.
async fn a_reopened_store_still_knows_all_of_it(path: &std::path::Path) {
    let engine = EmbeddedMade::open(path).expect("the durable store reopens");

    let head = engine
        .get_agentic_system(&system_id(SYSTEM_ID), None)
        .await
        .expect("the design survived");
    assert!(head.revision().get() >= 2);

    let first = engine
        .get_agentic_system(&system_id(SYSTEM_ID), Some(AgenticSystemRevision::INITIAL))
        .await
        .expect("the first revision survived");
    assert_eq!(
        first.system().purpose().as_str(),
        "write a draft and have somebody read it"
    );

    let run = engine
        .get_agentic_system_execution(&AgenticSystemExecutionId::new(RUN_ID).unwrap())
        .await
        .expect("the run survived");
    assert_eq!(run.execution().system().revision().get(), 2);
    assert_eq!(run.ceremonies().len(), 2);
}

fn document(design: Value) -> AgenticSystemDesignDocument {
    serde_json::from_value(design).expect("the walkthrough writes a decodable document")
}

fn system_id(raw: &str) -> AgenticSystemId {
    AgenticSystemId::new(raw).unwrap()
}

fn ceremony(raw: &str) -> SystemCeremonyId {
    SystemCeremonyId::new(raw).unwrap()
}

fn instantiation() -> InstantiateAgenticSystemInput {
    InstantiateAgenticSystemInput::new(
        system_id(SYSTEM_ID),
        AgenticSystemRevision::new(2).unwrap(),
        AgenticSystemExecutionId::new(RUN_ID).unwrap(),
        [(ceremony("drafting"), CeremonyContext::default())],
        // Only the author is offered. The reviewer is not, and the
        // run has to say so rather than find a substitute.
        [
            (
                made_core::value_objects::ParticipantId::new("writer").unwrap(),
                ParticipantOffer::new(
                    Specialty::new("author").unwrap(),
                    [Capability::new("drafting").unwrap()],
                ),
            ),
            (
                made_core::value_objects::ParticipantId::new("operator").unwrap(),
                ParticipantOffer::new(Specialty::new("operator").unwrap(), []),
            ),
        ],
        None,
        "walkthrough-host",
        AuditActorKind::Service,
    )
}

fn design() -> Value {
    json!({
        "id": SYSTEM_ID,
        "purpose": "write a draft and have somebody read it",
        "integrator_role_id": "integrator",
        "roles": [
            {"id": "integrator", "responsibility": "asks for the work and answers for it", "kind": "integrator"},
            {"id": "author", "responsibility": "writes the draft", "kind": "contributor"},
            {"id": "reviewer", "responsibility": "reads it critically", "kind": "reviewer"},
        ],
        "participants": [
            {"id": "operator", "role": "integrator", "kind": "person"},
            {
                "id": "writer",
                "role": "author",
                "kind": "agent",
                "binding": {"capabilities": ["drafting"], "independence_group": "writing-desk"},
            },
            {
                "id": "critic",
                "role": "reviewer",
                "kind": "agent",
                "binding": {"capabilities": ["review"], "independence_group": "reading-desk"},
            },
        ],
        "topology": [
            {"from": "operator", "to": "writer", "kind": "coordination", "handoff": true},
            {"from": "writer", "to": "critic", "kind": "execution"},
        ],
        "profiles": {
            "author": {
                "requested_model": "balanced-model",
                "requested_reasoning_effort": "medium",
                "required_capabilities": ["drafting"],
                "fallback_policy": "fallback",
                "fallback_model": "small-model",
            }
        },
        "ceremonies": [
            {
                "id": "drafting",
                "pin": {"name": "system_drafting", "version": "1.0"},
                "purpose": "produce the draft",
                "activation": {"kind": "manual"},
                "role_bindings": {"AUTHOR": "writer", "EDITOR": "operator"},
            },
            {
                "id": "review",
                "pin": {"name": "system_review", "version": "1.0"},
                "purpose": "read the draft critically",
                "depends_on": ["drafting"],
                "activation": {"kind": "after_dependencies"},
                "role_bindings": {"REVIEWER": "critic", "AUTHOR": "writer"},
            },
        ],
        "supervision": {
            "independence": [{"reviewer": "reviewer", "reviewed": "author"}]
        },
    })
}
