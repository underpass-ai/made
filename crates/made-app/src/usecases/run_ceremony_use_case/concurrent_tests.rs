use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{CeremonyDefinition, CeremonyEvent};
use made_core::error::DomainError;
use made_core::ports::{CeremonyStepHandlerPort, CeremonyStepHandlerRequest};
use made_core::value_objects::{
    Attributes, AuditActorKind, CeremonyChildSpawn, CeremonyChildSpec, CeremonyContext,
    CeremonyGuard, CeremonyName, CeremonyRole, CeremonyState, CeremonyStep, CeremonyTransition,
    CeremonyVersion, ContextKey, DynamicRoleBinding, GuardCondition, GuardName, MaxChildDepth,
    MaxChildren, MaxParallel, RetryPolicy, RoleAction, RoleId, StateExecution, StateId,
    StateIteration, StateRepeatPolicy, StateRepeatUntilCondition, StepHandlerConfig,
    StepHandlerKind, StepId, StepOutput, StepOutputField, StepResult, TransitionTrigger,
};
use serde_json::json;
use tokio::sync::{Barrier, Mutex};

use super::*;
use crate::usecases::ceremony_test_support::{
    a_memory, ceremony_id, lease_owner, lease_ttl, now, resolver_with, review_child_definition,
    stream_over, DefinitionRepositoryFake, EventStoreFake, FixedClock, PublicationsFake,
};
use crate::usecases::PrepareCeremonyChildrenUseCase;

struct SnapshotReadFault {
    store: Arc<EventStoreFake>,
    failed: std::sync::atomic::AtomicBool,
}

#[async_trait]
impl made_core::ports::CeremonySnapshotStorePort for SnapshotReadFault {
    async fn save(&self, snapshot: made_core::ports::CeremonySnapshot) -> Result<(), DomainError> {
        made_core::ports::CeremonySnapshotStorePort::save(self.store.as_ref(), snapshot).await
    }

    async fn latest(
        &self,
        id: &made_core::value_objects::CeremonyId,
    ) -> Result<Option<made_core::ports::CeremonySnapshot>, DomainError> {
        let snapshot = self.store.latest(id).await?;
        let active = snapshot.as_ref().map_or(0, |snapshot| {
            snapshot
                .instance
                .step_records()
                .values()
                .filter(|record| record.has_live_lease_at(now()))
                .count()
        });
        if active == 2 && !self.failed.swap(true, Ordering::SeqCst) {
            return Err(DomainError::InvariantViolated {
                reason: "injected post-claim read failure",
            });
        }
        Ok(snapshot)
    }

    async fn forget(&self, id: &made_core::value_objects::CeremonyId) -> Result<(), DomainError> {
        self.store.forget(id).await
    }
}

#[tokio::test]
async fn reload_failure_after_claim_drains_every_accepted_sibling() {
    let definition = concurrent_definition(3, GuardCondition::AllStepsCompleted);
    let store = Arc::new(EventStoreFake::default());
    let stream = Arc::new(crate::services::SessionStream::new(
        store.clone(),
        Arc::new(SnapshotReadFault {
            store: store.clone(),
            failed: false.into(),
        }),
        Arc::new(made_core::ports::NoopCeremonyEventSubscriber),
    ));
    let handler = GatedHandler::new(0, []);
    let usecase = RunCeremonyUseCase::new(
        Arc::new(DefinitionRepositoryFake::new(definition.clone())),
        stream.clone(),
        handler.clone(),
        Arc::new(FixedClock::new(now())),
    );
    let error = usecase
        .execute(RunCeremonyInput::new(
            ceremony_id(),
            definition,
            CeremonyContext::empty(),
            lease_owner(),
            lease_ttl(),
            "driver-test",
            AuditActorKind::Agent,
        ))
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        DomainError::InvariantViolated {
            reason: "injected post-claim read failure"
        }
    ));
    assert_eq!(handler.calls.lock().await.len(), 2);
    let records = stream.records(&ceremony_id()).await.unwrap();
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::StepStarted(_))))
            .count(),
        2
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::StepCompleted(_))))
            .count(),
        2
    );
    assert!(!records
        .iter()
        .any(|record| matches!(record.event(), Some(CeremonyEvent::TransitionApplied(_)))));
    assert!(stream
        .load(&ceremony_id())
        .await
        .unwrap()
        .instance
        .step_records()
        .values()
        .all(|record| !record.has_live_lease_at(now())));
}

struct GatedHandler {
    barrier: Arc<Barrier>,
    gated_calls: usize,
    started: AtomicUsize,
    active: AtomicUsize,
    peak: AtomicUsize,
    calls: Mutex<Vec<StepId>>,
    failing: BTreeSet<StepId>,
}

impl GatedHandler {
    fn new(gated_calls: usize, failing: impl IntoIterator<Item = StepId>) -> Arc<Self> {
        Arc::new(Self {
            barrier: Arc::new(Barrier::new(gated_calls + 1)),
            gated_calls,
            started: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            calls: Mutex::new(Vec::new()),
            failing: failing.into_iter().collect(),
        })
    }

    async fn release_first_wave(&self) {
        self.barrier.wait().await;
    }
}

#[async_trait]
impl CeremonyStepHandlerPort for GatedHandler {
    async fn execute(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        let ordinal = self.started.fetch_add(1, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        self.calls.lock().await.push(request.step_id().clone());
        if ordinal < self.gated_calls {
            self.barrier.wait().await;
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        if self.failing.contains(request.step_id()) {
            return Err(DomainError::InvariantViolated {
                reason: "controlled sibling failure",
            });
        }
        StepResult::completed(StepOutput::empty())
    }
}

fn concurrent_definition(step_count: usize, join: GuardCondition) -> CeremonyDefinition {
    let work = StateId::new("work").unwrap();
    let done = StateId::new("done").unwrap();
    let finish = TransitionTrigger::new("finish").unwrap();
    let step_ids = (0..step_count)
        .map(|index| StepId::new(format!("step_{index}")).unwrap())
        .collect::<Vec<_>>();
    let steps = step_ids
        .iter()
        .map(|id| {
            CeremonyStep::new(
                id.clone(),
                work.clone(),
                StepHandlerKind::new("controlled").unwrap(),
                StepHandlerConfig::empty(),
                RetryPolicy::single_attempt(),
                None,
            )
        })
        .collect::<Vec<_>>();
    let mut roles = step_ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let mut actions = vec![RoleAction::step(id.clone())];
            if index == 0 {
                actions.push(RoleAction::transition(finish.clone()));
            }
            CeremonyRole::new(RoleId::new(format!("role_{index}")).unwrap(), actions).unwrap()
        })
        .collect::<Vec<_>>();
    roles.shrink_to_fit();
    CeremonyDefinition::new(
        CeremonyName::new("automatic_fanout").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(work.clone()).with_execution(StateExecution::Concurrent),
            CeremonyState::terminal(done.clone()),
        ],
        vec![
            CeremonyTransition::new(work, done, finish, vec![GuardName::new("joined").unwrap()])
                .unwrap(),
        ],
        steps,
        vec![CeremonyGuard::new(GuardName::new("joined").unwrap(), join)],
        roles,
    )
    .unwrap()
    .with_max_parallel(MaxParallel::new(3).unwrap())
}

fn concurrent_spawn_definition() -> CeremonyDefinition {
    let base = concurrent_definition(2, GuardCondition::AllStepsCompleted);
    let spawn = CeremonyChildSpawn::new(
        vec![CeremonyChildSpec::new(
            CeremonyName::new("review_child").unwrap(),
            CeremonyVersion::v1(),
            std::collections::BTreeMap::new(),
        )],
        MaxChildren::new(1).unwrap(),
        MaxChildDepth::new(2).unwrap(),
    )
    .unwrap();
    CeremonyDefinition::new(
        CeremonyName::new("concurrent_child_fanout").unwrap(),
        base.version().clone(),
        None,
        Vec::new(),
        Vec::new(),
        base.states().values().cloned(),
        base.transitions().iter().cloned(),
        base.steps_in_declaration_order()
            .cloned()
            .map(|step| step.with_spawn(spawn.clone())),
        base.guards().values().cloned(),
        base.roles().values().cloned(),
    )
    .unwrap()
    .with_max_parallel(MaxParallel::new(2).unwrap())
}

fn repeating_definition() -> CeremonyDefinition {
    let base = concurrent_definition(2, GuardCondition::AllStepsCompleted);
    CeremonyDefinition::new(
        base.name().clone(),
        base.version().clone(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(StateId::new("work").unwrap())
                .with_execution(StateExecution::Concurrent)
                .with_repeat_policy(StateRepeatPolicy::new(
                    StateIteration::new(2).unwrap(),
                    StateRepeatUntilCondition::new(
                        StepId::new("step_0").unwrap(),
                        StepOutputField::new("ready").unwrap(),
                        json!(true),
                    ),
                )),
            CeremonyState::terminal(StateId::new("done").unwrap()),
        ],
        base.transitions().iter().cloned(),
        base.steps_in_declaration_order().cloned(),
        base.guards().values().cloned(),
        base.roles().values().cloned(),
    )
    .unwrap()
    .with_max_parallel(MaxParallel::new(2).unwrap())
}

fn dynamic_same_role_definition() -> CeremonyDefinition {
    let work = StateId::new("work").unwrap();
    let done = StateId::new("done").unwrap();
    let role = RoleId::new("reviewer").unwrap();
    let steps = ["dynamic_a", "dynamic_b"]
        .into_iter()
        .map(|id| {
            CeremonyStep::new(
                StepId::new(id).unwrap(),
                work.clone(),
                StepHandlerKind::new("controlled").unwrap(),
                StepHandlerConfig::empty(),
                RetryPolicy::single_attempt(),
                None,
            )
            .with_dynamic_role_binding(
                DynamicRoleBinding::new(ContextKey::new("next_role").unwrap(), [role.clone()])
                    .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let finish = TransitionTrigger::new("finish").unwrap();
    CeremonyDefinition::new(
        CeremonyName::new("dynamic_fanout").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(work.clone()).with_execution(StateExecution::Concurrent),
            CeremonyState::terminal(done.clone()),
        ],
        vec![CeremonyTransition::new(
            work,
            done,
            finish.clone(),
            vec![GuardName::new("joined").unwrap()],
        )
        .unwrap()],
        steps.clone(),
        vec![CeremonyGuard::new(
            GuardName::new("joined").unwrap(),
            GuardCondition::AllStepsCompleted,
        )],
        vec![CeremonyRole::new(
            role,
            [
                RoleAction::step(steps[0].id().clone()),
                RoleAction::step(steps[1].id().clone()),
                RoleAction::transition(finish),
            ],
        )
        .unwrap()],
    )
    .unwrap()
    .with_max_parallel(MaxParallel::new(2).unwrap())
}

struct RepeatingHandler {
    ready_calls: AtomicUsize,
}

#[async_trait]
impl CeremonyStepHandlerPort for RepeatingHandler {
    async fn execute(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        let output = if request.step_id() == &StepId::new("step_0").unwrap() {
            let ready = self.ready_calls.fetch_add(1, Ordering::SeqCst) > 0;
            StepOutput::new(
                Attributes::new(std::collections::BTreeMap::from([(
                    "ready".to_owned(),
                    json!(ready),
                )]))
                .unwrap(),
            )
        } else {
            StepOutput::empty()
        };
        StepResult::completed(output)
    }
}

struct NeverReadyHandler;

#[async_trait]
impl CeremonyStepHandlerPort for NeverReadyHandler {
    async fn execute(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        let output = if request.step_id() == &StepId::new("step_0").unwrap() {
            StepOutput::new(
                Attributes::new(std::collections::BTreeMap::from([(
                    "ready".to_owned(),
                    json!(false),
                )]))
                .unwrap(),
            )
        } else {
            StepOutput::empty()
        };
        StepResult::completed(output)
    }
}

fn run_with(
    definition: CeremonyDefinition,
    handler: Arc<dyn CeremonyStepHandlerPort>,
    ceiling: u8,
) -> (
    Arc<EventStoreFake>,
    tokio::task::JoinHandle<Result<RunCeremonyOutput, DomainError>>,
) {
    let store = Arc::new(EventStoreFake::default());
    let (stream, _) = stream_over(store.clone());
    let usecase = Arc::new(
        RunCeremonyUseCase::new(
            Arc::new(DefinitionRepositoryFake::new(definition.clone())),
            stream,
            handler,
            Arc::new(FixedClock::new(now())),
        )
        .with_max_parallel_ceiling(MaxParallel::new(ceiling).unwrap()),
    );
    let task = tokio::spawn(async move {
        usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition,
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "driver-test",
                AuditActorKind::Agent,
            ))
            .await
    });
    (store, task)
}

#[tokio::test]
async fn concurrent_state_overlaps_a_bounded_batch_and_stops_at_an_early_join() {
    let definition = concurrent_definition(3, GuardCondition::AnyStepCompleted);
    let handler = GatedHandler::new(2, []);
    let (_, task) = run_with(definition.clone(), handler.clone(), 2);

    handler.release_first_wave().await;
    let output = task.await.unwrap().unwrap();

    assert!(output.instance().is_completed(&definition));
    assert_eq!(handler.peak.load(Ordering::SeqCst), 2);
    assert_eq!(handler.calls.lock().await.len(), 2);
    assert_eq!(output.step_traces().len(), 2);
}

#[tokio::test]
async fn concurrent_spawn_batch_opens_every_child_without_invoking_the_handler() {
    let definition = concurrent_spawn_definition();
    let definitions = Arc::new(DefinitionRepositoryFake::default());
    let publications = Arc::new(PublicationsFake::default());
    publications.seed(review_child_definition()).await;
    let store = Arc::new(EventStoreFake::default());
    let (stream, _) = stream_over(store.clone());
    let children = Arc::new(PrepareCeremonyChildrenUseCase::new(
        resolver_with(definitions.clone(), publications.clone()),
        publications,
        stream.clone(),
        Arc::new(FixedClock::new(now())),
        a_memory(),
    ));
    let handler = GatedHandler::new(0, []);
    let usecase = RunCeremonyUseCase::new(
        definitions,
        stream,
        handler.clone(),
        Arc::new(FixedClock::new(now())),
    )
    .with_max_parallel_ceiling(MaxParallel::new(2).unwrap())
    .with_child_orchestrator(children);

    let output = usecase
        .execute(RunCeremonyInput::new(
            ceremony_id(),
            definition.clone(),
            CeremonyContext::empty(),
            lease_owner(),
            lease_ttl(),
            "driver-test",
            AuditActorKind::Agent,
        ))
        .await
        .unwrap();

    assert!(output.instance().is_completed(&definition));
    assert!(handler.calls.lock().await.is_empty());
    assert_eq!(output.instance().child_groups().len(), 2);
    for group in output.instance().child_groups().values() {
        assert_eq!(group.plan().children().len(), 1);
        assert!(store.exists(group.plan().children()[0].child_id()).await);
    }
    let records = store.records(&ceremony_id()).await;
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::ChildSpawnPlanned(_))))
            .count(),
        2
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::StepCompleted(_))))
            .count(),
        2
    );
}

#[tokio::test]
async fn host_ceiling_one_keeps_all_join_execution_serial() {
    let definition = concurrent_definition(3, GuardCondition::AllStepsCompleted);
    let handler = GatedHandler::new(1, []);
    let (_, task) = run_with(definition.clone(), handler.clone(), 1);

    handler.release_first_wave().await;
    let output = task.await.unwrap().unwrap();

    assert!(output.instance().is_completed(&definition));
    assert_eq!(handler.peak.load(Ordering::SeqCst), 1);
    assert_eq!(handler.calls.lock().await.len(), 3);
}

#[tokio::test]
async fn sibling_failure_drains_every_claimed_completion_before_returning() {
    let definition = concurrent_definition(2, GuardCondition::AllStepsCompleted);
    let failed = StepId::new("step_0").unwrap();
    let handler = GatedHandler::new(2, [failed]);
    let (store, task) = run_with(definition, handler.clone(), 2);

    handler.release_first_wave().await;
    let error = task.await.unwrap().unwrap_err();

    assert!(error.to_string().contains("did not complete successfully"));
    assert_eq!(handler.calls.lock().await.len(), 2);
    let records = store.records(&ceremony_id()).await;
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::StepFailed(_))))
            .count(),
        1
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::StepCompleted(_))))
            .count(),
        1
    );
}

#[tokio::test]
async fn concurrent_state_repeat_runs_a_fresh_bounded_batch() {
    let definition = repeating_definition();
    let handler = Arc::new(RepeatingHandler {
        ready_calls: AtomicUsize::new(0),
    });
    let (_, task) = run_with(definition.clone(), handler, 2);

    let output = task.await.unwrap().unwrap();

    assert!(output.instance().is_completed(&definition));
    assert_eq!(output.step_traces().len(), 4);
    assert_eq!(output.step_traces()[0].state_iteration().get(), 1);
    assert_eq!(output.step_traces()[2].state_iteration().get(), 2);
}

#[tokio::test]
async fn concurrent_state_reports_repeat_limit_before_joining_terminal() {
    let definition = repeating_definition();
    let (store, task) = run_with(definition, Arc::new(NeverReadyHandler), 2);

    let error = task.await.unwrap().unwrap_err();

    assert!(error
        .to_string()
        .contains("ceremony state repeat limit exhausted"));
    let records = store.records(&ceremony_id()).await;
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::TransitionApplied(_))))
            .count(),
        0
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(
                record.event(),
                Some(CeremonyEvent::StateIterationStarted(_))
            ))
            .count(),
        1
    );
}

#[tokio::test]
async fn dynamic_same_role_claim_refusal_still_drains_the_accepted_sibling() {
    let definition = dynamic_same_role_definition();
    let handler = GatedHandler::new(1, []);
    let store = Arc::new(EventStoreFake::default());
    let (stream, _) = stream_over(store.clone());
    let usecase = Arc::new(
        RunCeremonyUseCase::new(
            Arc::new(DefinitionRepositoryFake::new(definition.clone())),
            stream,
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        )
        .with_max_parallel_ceiling(MaxParallel::new(2).unwrap()),
    );
    let task = tokio::spawn(async move {
        usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition,
                CeremonyContext::new(
                    Attributes::new(std::collections::BTreeMap::from([(
                        "next_role".to_owned(),
                        json!("reviewer"),
                    )]))
                    .unwrap(),
                ),
                lease_owner(),
                lease_ttl(),
                "driver-test",
                AuditActorKind::Agent,
            ))
            .await
    });

    handler.release_first_wave().await;
    let error = task.await.unwrap().unwrap_err();
    assert!(error
        .to_string()
        .contains("role is already assigned to another step"));
    assert_eq!(handler.calls.lock().await.len(), 1);
    let records = store.records(&ceremony_id()).await;
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record.event(), Some(CeremonyEvent::StepCompleted(_))))
            .count(),
        1
    );
}
