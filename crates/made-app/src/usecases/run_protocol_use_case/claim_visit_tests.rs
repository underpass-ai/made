use std::sync::Arc;

use made_core::entities::{AuditFact, ProtocolDefinition, ProtocolEvent};
use made_core::value_objects::{
    AuditActorKind, MaxTransitions, ProtocolContext, ProtocolState, ProtocolTransition, RoleAction,
    RoleId, StateId, StateVisit, StepOutput, StepResult, TransitionTrigger,
};

use super::*;
use crate::usecases::protocol_test_support::{
    definition, lease_owner, lease_ttl, now, protocol_id, started_instance, step_id,
    stream_overtaken_on_step_claim, DefinitionRepositoryFake, EventStoreFake, FixedClock,
    StepHandlerFake,
};

fn revisitable_definition() -> ProtocolDefinition {
    let base = definition();
    let mut states = base.states().values().cloned().collect::<Vec<_>>();
    let state_a = base.initial_state_id().clone();
    let state_b = StateId::new("WAITING").unwrap();
    states.push(ProtocolState::intermediate(state_b.clone()));
    let mut transitions = base.transitions().to_vec();
    transitions.extend([
        ProtocolTransition::new(
            state_a.clone(),
            state_b.clone(),
            TransitionTrigger::new("leave").unwrap(),
            Vec::new(),
        )
        .unwrap(),
        ProtocolTransition::new(
            state_b,
            state_a,
            TransitionTrigger::new("return").unwrap(),
            Vec::new(),
        )
        .unwrap(),
    ]);
    let mut roles = base.roles().values().cloned().collect::<Vec<_>>();
    roles.push(
        made_core::value_objects::ProtocolRole::new(
            RoleId::new("DRIVER").unwrap(),
            [
                RoleAction::transition(TransitionTrigger::new("leave").unwrap()),
                RoleAction::transition(TransitionTrigger::new("return").unwrap()),
            ],
        )
        .unwrap(),
    );
    ProtocolDefinition::new_with_transition_budgets(
        base.name().clone(),
        base.version().clone(),
        None,
        Vec::new(),
        Vec::new(),
        states,
        transitions,
        base.steps_in_declaration_order().cloned(),
        base.guards().values().cloned(),
        roles,
        Some(MaxTransitions::new(3).unwrap()),
        None,
    )
    .unwrap()
}

fn concurrent_revisit(definition: &ProtocolDefinition) -> Vec<AuditFact> {
    let mut instance = started_instance(definition);
    let actor =
        session_facts::seat(&RoleId::new("DRIVER").unwrap(), AuditActorKind::Agent).unwrap();
    let mut facts = Vec::new();
    for trigger in ["leave", "return"] {
        let events = instance
            .decide(
                &ProtocolCommand::ApplyTransition(ApplyTransition {
                    role_id: Some(RoleId::new("DRIVER").unwrap()),
                    trigger: TransitionTrigger::new(trigger).unwrap(),
                    now: now(),
                }),
                definition,
            )
            .unwrap();
        facts.extend(session_facts::facts(&instance, events.clone(), &actor, now()).unwrap());
        for event in events {
            instance.apply(&event);
        }
    }
    facts
}

#[tokio::test]
async fn claim_retry_trace_uses_the_visit_of_the_accepted_claim() {
    let definition = revisitable_definition();
    let store = Arc::new(EventStoreFake::default());
    let handler = Arc::new(StepHandlerFake::succeeding(
        StepResult::completed(StepOutput::empty()).unwrap(),
    ));
    let usecase = RunProtocolUseCase::new(
        Arc::new(DefinitionRepositoryFake::new(definition.clone())),
        stream_overtaken_on_step_claim(store.clone(), concurrent_revisit(&definition)),
        handler.clone(),
        Arc::new(FixedClock::new(now())),
    );
    let output = usecase
        .execute(RunProtocolInput::new(
            protocol_id(),
            definition.clone(),
            ProtocolContext::empty(),
            lease_owner(),
            lease_ttl(),
            "test-driver",
            AuditActorKind::Agent,
        ))
        .await
        .unwrap();

    assert!(output.instance().is_completed(&definition));
    assert_eq!(handler.requests().await.len(), 1);
    assert_eq!(output.instance().transitions().len(), 3);
    assert_eq!(output.step_traces().len(), 1);
    let accepted_visit = StateVisit::new(3).unwrap();
    let trace = &output.step_traces()[0];
    assert_eq!(trace.state_visit(), accepted_visit);
    assert_eq!(trace.state_iteration().get(), 1);
    assert_eq!(
        output
            .instance()
            .step_record(&step_id())
            .unwrap()
            .state_visit(),
        accepted_visit
    );
    let records = store.records(&protocol_id()).await;
    let claims = records
        .iter()
        .filter_map(|record| match record.event() {
            Some(ProtocolEvent::StepStarted(event)) => Some(event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].state_visit, Some(accepted_visit));
    let transcript = protocol_transcript_projection::transcript(&records);
    assert_eq!(transcript.contributions().len(), 1);
    assert_eq!(transcript.contributions()[0].state_visit(), accepted_visit);
}
