//! The integrator loop through the facade an operator actually opens.
//!
//! Composing the pieces is not the same as connecting them. Everything
//! here goes through the engine: a binding is taken, real work is
//! sealed, and the loop's paperwork is read. Nothing enqueues a
//! delivery by hand, which is exactly what the assertion is about — a
//! deployment where the projection was built and never woken would
//! pass every unit test and hand a bound host nothing, for ever.

use std::collections::BTreeMap;

use made_app::usecases::integrator::{BindCeremonyIntegratorInput, ListAttentionDeliveriesInput};
use made_app::usecases::{CompleteCeremonyStepInput, StartCeremonyInput, StartCeremonyStepInput};
use made_core::ports::BindReplacement;
use made_core::value_objects::{
    AttentionKind, Attributes, AuditActorKind, CeremonyContext, CeremonyId, DurationMs,
    FollowReplacement, HostActivationMode, HostAddress, HostAgentIncarnation, HostDeliveryItem,
    HostDeliveryItemKind, HostDestination, HostKind, IdempotencyKey, IntegratorBindingId,
    IntegratorScope, LeaseOwnerId, RoleId, StepId, StepOutput, StepResult,
};
use made_embedded::EmbeddedMade;
use serde_json::json;

const DEFINITION: &str = include_str!("../../../tests/e2e/ceremonies/state-visits.yaml");

fn ceremony() -> CeremonyId {
    CeremonyId::new("integrator-loop").expect("a valid ceremony id")
}

fn binding_id() -> IntegratorBindingId {
    IntegratorBindingId::new("b-1").expect("a valid binding id")
}

fn bind_input() -> BindCeremonyIntegratorInput {
    BindCeremonyIntegratorInput {
        binding_id: binding_id(),
        scope: IntegratorScope::ceremony(ceremony()),
        role_id: RoleId::new("INTEGRATOR").expect("a valid role"),
        destination: HostDestination::new(
            HostKind::new("claude-code").expect("a valid host kind"),
            HostAddress::new("session-1").expect("a valid address"),
            HostActivationMode::None,
        ),
        incarnation: HostAgentIncarnation::new("run-1").expect("a valid incarnation"),
        replacement: BindReplacement::Refuse,
        follow: FollowReplacement::Stay,
    }
}

/// Bind an integrator, then seal one step result through the engine.
async fn drive(engine: &EmbeddedMade) {
    let mounted = engine
        .mount_yaml(DEFINITION)
        .await
        .expect("the yaml mounts");
    let definition = &mounted.definitions()[0];
    engine
        .start(StartCeremonyInput::new(
            ceremony(),
            definition.name().clone(),
            definition.version().clone(),
            CeremonyContext::empty(),
            "operator",
            AuditActorKind::Service,
        ))
        .await
        .expect("the ceremony starts");
    engine
        .integrator_loop()
        .bind()
        .execute(bind_input())
        .await
        .expect("the binding is taken");

    let step = StepId::new("a").expect("a valid step id");
    let claimed = engine
        .start_step(StartCeremonyStepInput::new(
            ceremony(),
            RoleId::new("DRIVER").expect("a valid role"),
            AuditActorKind::Agent,
            step.clone(),
            LeaseOwnerId::new("host").expect("a valid owner"),
            IdempotencyKey::new("1-1-a").expect("a valid key"),
            DurationMs::from_millis(60_000),
        ))
        .await
        .expect("the step is claimed");
    let output = StepOutput::new(
        Attributes::new(BTreeMap::from([
            ("ready".to_owned(), json!(true)),
            ("marker".to_owned(), json!("1-1")),
        ]))
        .expect("valid attributes"),
    );
    engine
        .complete_step(CompleteCeremonyStepInput::new(
            ceremony(),
            step,
            StepResult::completed(output).expect("a valid result"),
            AuditActorKind::Agent,
            claimed.claim_fence().clone(),
        ))
        .await
        .expect("the result is sealed");
}

/// What the loop was offered for that work, read back through the
/// composed use case.
async fn offered(engine: &EmbeddedMade) -> Vec<AttentionKind> {
    let page = engine
        .integrator_loop()
        .deliveries()
        .execute(ListAttentionDeliveriesInput {
            binding_id: Some(binding_id()),
            ..ListAttentionDeliveriesInput::default()
        })
        .await
        .expect("the ledger reads");
    page.records()
        .iter()
        .inspect(|record| assert_eq!(record.item().kind(), HostDeliveryItemKind::Attention))
        .filter_map(|record| match record.item() {
            HostDeliveryItem::Attention { attention_id, .. } => {
                AttentionKind::from_label(attention_id.as_str().rsplit(':').next()?)
            }
            HostDeliveryItem::Intervention { .. } => None,
        })
        .collect()
}

#[tokio::test]
async fn sealing_a_result_offers_it_to_the_bound_integrator_in_memory() {
    let engine = EmbeddedMade::builder().build();

    drive(&engine).await;

    assert!(
        offered(&engine)
            .await
            .contains(&AttentionKind::ResultAvailable),
        "nobody enqueued this: the append has to have woken the projection"
    );
}

#[tokio::test]
async fn sealing_a_result_offers_it_to_the_bound_integrator_durably() {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp");
    std::fs::create_dir_all(&scratch).expect("the scratch directory exists");
    let directory = tempfile::tempdir_in(scratch).expect("a temporary state directory");
    let path = directory.path().join("integrator-loop.sqlite3");

    {
        let engine = EmbeddedMade::open(&path).expect("the durable engine opens");
        drive(&engine).await;
    }

    // Reopened, because the point of a durable ledger is that the offer
    // outlives the process that derived it.
    let reopened = EmbeddedMade::open(&path).expect("the durable engine reopens");
    assert!(
        offered(&reopened)
            .await
            .contains(&AttentionKind::ResultAvailable),
        "an offer that did not survive the restart is a loop that cannot recover"
    );
}
