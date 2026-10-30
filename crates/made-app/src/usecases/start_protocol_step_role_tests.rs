use std::collections::BTreeMap;
use std::sync::Arc;

use made_core::entities::protocol_events::ContextWritten;
use made_core::entities::{ProtocolDefinition, ProtocolEvent, ProtocolInstance};
use made_core::error::DomainError;
use made_core::value_objects::{
    Attributes, AuditActorKind, AuditEventType, ContextKey, ContextPatch, DynamicRoleBinding,
    GuardCondition, GuardName, ProtocolContext, ProtocolGuard, ProtocolName, ProtocolRole,
    ProtocolState, ProtocolStep, ProtocolTransition, ProtocolVersion, RetryPolicy, RoleAction,
    RoleId, StateExecution, StateId, StateIteration, StepAttempt, StepHandlerConfig,
    StepHandlerKind, StepId, StepIteration, TransitionTrigger,
};

use super::protocol_test_support::{
    definition_resolver, idempotency_key, lease_owner, lease_ttl, now, protocol_id, stream,
    stream_overtaken_once, DefinitionRepositoryFake, EventStoreFake, FixedClock,
};
use super::{StartProtocolStepInput, StartProtocolStepUseCase};
use crate::services::session_facts;

#[tokio::test]
async fn automatic_claim_re_resolves_role_and_actor_after_context_wins_the_race() {
    let definition = dynamic_role_definition();
    let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
    let instances = Arc::new(EventStoreFake::default());
    let instance = dynamic_role_instance(&definition, "B");
    instances.save(&instance).await.unwrap();
    let context_won = ProtocolEvent::ContextWritten(ContextWritten {
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
    let usecase = StartProtocolStepUseCase::new(
        definition_resolver(definitions),
        stream_overtaken_once(instances.clone(), overtaking_fact),
        Arc::new(FixedClock::new(now())),
    );

    usecase
        .execute(
            StartProtocolStepInput::new(
                protocol_id(),
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

    let records = instances.records(&protocol_id()).await;
    let started = records
        .iter()
        .find(|record| record.event_type() == AuditEventType::StepStarted)
        .unwrap();
    let Some(ProtocolEvent::StepStarted(event)) = started.event() else {
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
    let usecase = StartProtocolStepUseCase::new(
        definition_resolver(definitions),
        stream(instances.clone()),
        Arc::new(FixedClock::new(now())),
    );

    let error = usecase
        .execute(StartProtocolStepInput::new(
            protocol_id(),
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
    let usecase = StartProtocolStepUseCase::new(
        definition_resolver(definitions),
        stream(instances.clone()),
        Arc::new(FixedClock::new(now())),
    );

    usecase
        .execute(
            StartProtocolStepInput::new(
                protocol_id(),
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

    let records = instances.records(&protocol_id()).await;
    let started = records
        .iter()
        .find(|record| record.event_type() == AuditEventType::StepStarted)
        .unwrap();
    let Some(ProtocolEvent::StepStarted(event)) = started.event() else {
        panic!("the claim sealed a StepStarted event");
    };
    let static_role = RoleId::new("A").unwrap();
    assert_eq!(event.started_by, static_role);
    assert_eq!(event.sealed_role.as_ref(), Some(&static_role));
    assert!(event.role_from.is_none());
    assert_eq!(started.actor().role_id(), Some(&static_role));
}

fn dynamic_role_definition() -> ProtocolDefinition {
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
            ProtocolRole::new(
                RoleId::new(role).unwrap(),
                [RoleAction::step(dynamic.clone())],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    roles.push(
        ProtocolRole::new(
            RoleId::new("A").unwrap(),
            [
                RoleAction::step(static_step.clone()),
                RoleAction::transition(finish.clone()),
            ],
        )
        .unwrap(),
    );
    ProtocolDefinition::new(
        ProtocolName::new("dynamic_claim").unwrap(),
        ProtocolVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            ProtocolState::initial(work.clone()).with_execution(StateExecution::Concurrent),
            ProtocolState::terminal(done.clone()),
        ],
        vec![ProtocolTransition::new(work.clone(), done, finish, vec![joined.clone()]).unwrap()],
        vec![
            ProtocolStep::new(
                dynamic,
                work.clone(),
                StepHandlerKind::new("host_callback").unwrap(),
                StepHandlerConfig::empty(),
                RetryPolicy::single_attempt(),
                None,
            )
            .with_dynamic_role_binding(binding),
            ProtocolStep::new(
                static_step,
                work,
                StepHandlerKind::new("host_callback").unwrap(),
                StepHandlerConfig::empty(),
                RetryPolicy::single_attempt(),
                None,
            ),
        ],
        vec![ProtocolGuard::new(joined, GuardCondition::AnyStepCompleted)],
        roles,
    )
    .unwrap()
}

fn dynamic_role_instance(definition: &ProtocolDefinition, next_role: &str) -> ProtocolInstance {
    let context = ProtocolContext::new(
        Attributes::new(BTreeMap::from([(
            "next_role".to_owned(),
            serde_json::json!(next_role),
        )]))
        .unwrap(),
    );
    ProtocolInstance::start(protocol_id(), definition, context, now()).unwrap()
}
