use std::collections::BTreeMap;
use std::sync::Arc;

use made_core::entities::ceremony_events::ContextWritten;
use made_core::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    Attributes, AuditActorKind, AuditEventType, CeremonyContext, CeremonyGuard, CeremonyName,
    CeremonyRole, CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion, ContextKey,
    ContextPatch, DynamicRoleBinding, GuardCondition, GuardName, RetryPolicy, RoleAction, RoleId,
    StateExecution, StateId, StateIteration, StepAttempt, StepHandlerConfig, StepHandlerKind,
    StepId, StepIteration, TransitionTrigger,
};

use super::ceremony_test_support::{
    ceremony_id, definition_resolver, idempotency_key, lease_owner, lease_ttl, now, stream,
    stream_overtaken_once, DefinitionRepositoryFake, EventStoreFake, FixedClock,
};
use super::{StartCeremonyStepInput, StartCeremonyStepUseCase};
use crate::services::session_facts;

#[tokio::test]
async fn automatic_claim_re_resolves_role_and_actor_after_context_wins_the_race() {
    let definition = dynamic_role_definition();
    let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
    let instances = Arc::new(EventStoreFake::default());
    let instance = dynamic_role_instance(&definition, "B");
    instances.save(&instance).await.unwrap();
    let context_won = CeremonyEvent::ContextWritten(ContextWritten {
        state_visit: None,
        step_id: StepId::new("selector").unwrap(),
        state_iteration: StateIteration::FIRST,
        iteration: StepIteration::FIRST,
        attempt: StepAttempt::FIRST,
        patch: ContextPatch::new(BTreeMap::from([(
            ContextKey::new("next_role").unwrap(),
            serde_json::json!("C"),
        )]))
        .unwrap(),
        written_at: now(),
    });
    let overtaking_fact = session_facts::fact(
        &instance,
        context_won,
        session_facts::party("selector", AuditActorKind::Service).unwrap(),
        now(),
    )
    .unwrap();
    let usecase = StartCeremonyStepUseCase::new(
        definition_resolver(definitions),
        stream_overtaken_once(instances.clone(), overtaking_fact),
        Arc::new(FixedClock::new(now())),
    );

    usecase
        .execute(
            StartCeremonyStepInput::new(
                ceremony_id(),
                RoleId::new("B").unwrap(),
                AuditActorKind::Agent,
                StepId::new("dynamic").unwrap(),
                lease_owner(),
                idempotency_key("dynamic-after-conflict"),
                lease_ttl(),
            )
            .with_automatic_role_resolution(),
        )
        .await
        .unwrap();

    let records = instances.records(&ceremony_id()).await;
    let started = records
        .iter()
        .find(|record| record.event_type() == AuditEventType::StepStarted)
        .unwrap();
    let Some(CeremonyEvent::StepStarted(event)) = started.event() else {
        panic!("the claim sealed a StepStarted event");
    };
    let winner = RoleId::new("C").unwrap();
    assert_eq!(event.started_by, winner);
    assert_eq!(started.actor().role_id(), Some(&winner));
}

#[tokio::test]
async fn explicit_claim_still_refuses_a_role_that_differs_from_dynamic_context() {
    let definition = dynamic_role_definition();
    let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
    let instances = Arc::new(EventStoreFake::default());
    instances
        .save(&dynamic_role_instance(&definition, "C"))
        .await
        .unwrap();
    let usecase = StartCeremonyStepUseCase::new(
        definition_resolver(definitions),
        stream(instances.clone()),
        Arc::new(FixedClock::new(now())),
    );

    let error = usecase
        .execute(StartCeremonyStepInput::new(
            ceremony_id(),
            RoleId::new("B").unwrap(),
            AuditActorKind::Agent,
            StepId::new("dynamic").unwrap(),
            lease_owner(),
            idempotency_key("explicit-mismatch"),
            lease_ttl(),
        ))
        .await
        .unwrap_err();

    assert!(matches!(error, DomainError::InvariantViolated { .. }));
    assert!(instances.facts().await.is_empty());
}

#[tokio::test]
async fn automatic_static_claim_in_a_mixed_state_keeps_the_definition_role() {
    let definition = dynamic_role_definition();
    let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
    let instances = Arc::new(EventStoreFake::default());
    instances
        .save(&dynamic_role_instance(&definition, "B"))
        .await
        .unwrap();
    let usecase = StartCeremonyStepUseCase::new(
        definition_resolver(definitions),
        stream(instances.clone()),
        Arc::new(FixedClock::new(now())),
    );

    usecase
        .execute(
            StartCeremonyStepInput::new(
                ceremony_id(),
                RoleId::new("A").unwrap(),
                AuditActorKind::Agent,
                StepId::new("static").unwrap(),
                lease_owner(),
                idempotency_key("automatic-static"),
                lease_ttl(),
            )
            .with_automatic_role_resolution(),
        )
        .await
        .unwrap();

    let records = instances.records(&ceremony_id()).await;
    let started = records
        .iter()
        .find(|record| record.event_type() == AuditEventType::StepStarted)
        .unwrap();
    let Some(CeremonyEvent::StepStarted(event)) = started.event() else {
        panic!("the claim sealed a StepStarted event");
    };
    let static_role = RoleId::new("A").unwrap();
    assert_eq!(event.started_by, static_role);
    assert_eq!(event.sealed_role.as_ref(), Some(&static_role));
    assert!(event.role_from.is_none());
    assert_eq!(started.actor().role_id(), Some(&static_role));
}

fn dynamic_role_definition() -> CeremonyDefinition {
    let work = StateId::new("WORK").unwrap();
    let done = StateId::new("DONE").unwrap();
    let dynamic = StepId::new("dynamic").unwrap();
    let static_step = StepId::new("static").unwrap();
    let finish = TransitionTrigger::new("finish").unwrap();
    let joined = GuardName::new("joined").unwrap();
    let binding = DynamicRoleBinding::new(
        ContextKey::new("next_role").unwrap(),
        [RoleId::new("B").unwrap(), RoleId::new("C").unwrap()],
    )
    .unwrap();
    let mut roles = ["B", "C"]
        .into_iter()
        .map(|role| {
            CeremonyRole::new(
                RoleId::new(role).unwrap(),
                [RoleAction::step(dynamic.clone())],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    roles.push(
        CeremonyRole::new(
            RoleId::new("A").unwrap(),
            [
                RoleAction::step(static_step.clone()),
                RoleAction::transition(finish.clone()),
            ],
        )
        .unwrap(),
    );
    CeremonyDefinition::new(
        CeremonyName::new("dynamic_claim").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(work.clone()).with_execution(StateExecution::Concurrent),
            CeremonyState::terminal(done.clone()),
        ],
        vec![CeremonyTransition::new(work.clone(), done, finish, vec![joined.clone()]).unwrap()],
        vec![
            CeremonyStep::new(
                dynamic,
                work.clone(),
                StepHandlerKind::new("host_callback").unwrap(),
                StepHandlerConfig::empty(),
                RetryPolicy::single_attempt(),
                None,
            )
            .with_dynamic_role_binding(binding),
            CeremonyStep::new(
                static_step,
                work,
                StepHandlerKind::new("host_callback").unwrap(),
                StepHandlerConfig::empty(),
                RetryPolicy::single_attempt(),
                None,
            ),
        ],
        vec![CeremonyGuard::new(joined, GuardCondition::AnyStepCompleted)],
        roles,
    )
    .unwrap()
}

fn dynamic_role_instance(definition: &CeremonyDefinition, next_role: &str) -> CeremonyInstance {
    let context = CeremonyContext::new(
        Attributes::new(BTreeMap::from([(
            "next_role".to_owned(),
            serde_json::json!(next_role),
        )]))
        .unwrap(),
    );
    CeremonyInstance::start(ceremony_id(), definition, context, now()).unwrap()
}
