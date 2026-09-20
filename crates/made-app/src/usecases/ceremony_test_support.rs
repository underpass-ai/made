use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{
    AuditFact, AuditRecord, CeremonyDefinition, CeremonyEvent, CeremonyInstance,
    PublicationOutcome, PublishedCeremonyDefinition,
};
use made_core::error::DomainError;
use made_core::ports::{
    seal_continuation, AppendOutcome, CeremonyDefinitionPublicationPort,
    CeremonyDefinitionRepositoryPort, CeremonyEventStorePort, CeremonyEventSubscriberPort,
    CeremonySnapshot, CeremonySnapshotStorePort, CeremonyStepHandlerPort,
    CeremonyStepHandlerRequest, ClockPort, MemoryReaderPort, MemoryRecollection,
    MemoryWriteOutcome, MemoryWriterPort, NoopCeremonyEventSubscriber, PositionedRecord,
};
use made_core::value_objects::{
    Attributes, AuditActorKind, AuditEventType, CeremonyChildSpawn, CeremonyChildSpec,
    CeremonyContext, CeremonyEventPageLimit, CeremonyGuard, CeremonyId, CeremonyName, CeremonyRole,
    CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion, ContextKey, ContextWrites,
    DurationMs, GlobalPosition, GuardCondition, GuardName, IdempotencyKey, LeaseOwnerId,
    MaxChildDepth, MaxChildren, MemoryCapabilities, MemoryCapability, MemoryEntry, MemoryEntryId,
    MemoryEntryKind, MemoryMoment, MemoryProvenance, MemoryRelation, MemoryScope, MemoryWrite,
    RepeatUntilCondition, RetryPolicy, RoleAction, RoleId, StateId, StateIteration,
    StateRepeatPolicy, StateRepeatUntilCondition, StepAttempt, StepHandlerConfig, StepHandlerKind,
    StepId, StepIteration, StepOutputField, StepRepeatPolicy, StepResult, StepStatus,
    StreamVersion, TransitionTrigger,
};
use serde_json::json;
use time::macros::datetime;
use time::OffsetDateTime;
use tokio::sync::RwLock;

use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use crate::services::{session_facts, SessionMemoryRecorder, SessionStream};

mod definition_repository_fake;
mod event_store_fake;
mod fixed_clock;
mod memory_that_is_out;
mod publications_fake;
mod recording_memory;
mod sequence_step_handler_fake;
mod step_handler_fake;
mod store_that_conflicts_once;
mod store_that_loses_every_race;

pub(super) use definition_repository_fake::DefinitionRepositoryFake;
pub(super) use event_store_fake::EventStoreFake;
pub(super) use fixed_clock::FixedClock;
pub(super) use memory_that_is_out::MemoryThatIsOut;
pub(super) use publications_fake::PublicationsFake;
pub(super) use recording_memory::RecordingMemory;
pub(super) use sequence_step_handler_fake::SequenceStepHandlerFake;
pub(super) use step_handler_fake::StepHandlerFake;
pub(super) use store_that_conflicts_once::StoreThatConflictsOnce;
pub(super) use store_that_loses_every_race::StoreThatLosesEveryRace;

impl FixedClock {
    pub(super) fn new(now: OffsetDateTime) -> Self {
        Self { now }
    }
}

impl ClockPort for FixedClock {
    fn now(&self) -> OffsetDateTime {
        self.now
    }
}

impl DefinitionRepositoryFake {
    pub(super) fn new(definition: CeremonyDefinition) -> Self {
        let mut inner = BTreeMap::new();
        inner.insert(
            (definition.name().clone(), definition.version().clone()),
            definition,
        );
        Self {
            inner: RwLock::new(inner),
        }
    }
}

#[async_trait]
impl CeremonyDefinitionRepositoryPort for DefinitionRepositoryFake {
    async fn save(&self, definition: &CeremonyDefinition) -> Result<(), DomainError> {
        self.inner.write().await.insert(
            (definition.name().clone(), definition.version().clone()),
            definition.clone(),
        );
        Ok(())
    }

    async fn get(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<CeremonyDefinition, DomainError> {
        self.inner
            .read()
            .await
            .get(&(name.clone(), version.clone()))
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "ceremony_definition",
            })
    }

    async fn list(&self) -> Result<Vec<CeremonyDefinition>, DomainError> {
        Ok(self.inner.read().await.values().cloned().collect())
    }
}

impl StepHandlerFake {
    pub(super) fn succeeding(result: StepResult) -> Self {
        Self {
            result: Ok(result),
            requests: RwLock::new(Vec::new()),
        }
    }

    pub(super) fn failing(error: DomainError) -> Self {
        Self {
            result: Err(error),
            requests: RwLock::new(Vec::new()),
        }
    }

    pub(super) async fn requests(&self) -> Vec<CeremonyStepHandlerRequest> {
        self.requests.read().await.clone()
    }
}

#[async_trait]
impl CeremonyStepHandlerPort for StepHandlerFake {
    async fn execute(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        self.requests.write().await.push(request);
        self.result.clone()
    }
}

impl SequenceStepHandlerFake {
    pub(super) fn new(results: impl IntoIterator<Item = StepResult>) -> Self {
        Self {
            results: RwLock::new(results.into_iter().collect()),
            requests: RwLock::new(Vec::new()),
        }
    }

    pub(super) async fn requests(&self) -> Vec<CeremonyStepHandlerRequest> {
        self.requests.read().await.clone()
    }
}

#[async_trait]
impl CeremonyStepHandlerPort for SequenceStepHandlerFake {
    async fn execute(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        self.requests.write().await.push(request);
        self.results
            .write()
            .await
            .pop_front()
            .ok_or(DomainError::InvariantViolated {
                reason: "sequence step handler exhausted",
            })
    }
}

pub(super) fn now() -> OffsetDateTime {
    datetime!(2026-06-06 12:00:00 UTC)
}

pub(super) fn definition_name() -> CeremonyName {
    CeremonyName::new("editorial_meeting").unwrap()
}

pub(super) fn approval_definition_name() -> CeremonyName {
    CeremonyName::new("approval_ceremony").unwrap()
}

pub(super) fn version() -> CeremonyVersion {
    CeremonyVersion::v1()
}

pub(super) fn ceremony_id() -> CeremonyId {
    CeremonyId::new("ceremony-1").unwrap()
}

pub(super) fn role_id() -> RoleId {
    RoleId::new("FACILITATOR").unwrap()
}

pub(super) fn respondent_role_id() -> RoleId {
    RoleId::new("TABLE_MEMBER").unwrap()
}

pub(super) fn step_id() -> StepId {
    StepId::new("roundtable").unwrap()
}

pub(super) fn trigger() -> TransitionTrigger {
    TransitionTrigger::new("meeting_done").unwrap()
}

pub(super) fn lease_owner() -> LeaseOwnerId {
    LeaseOwnerId::new("runner-1").unwrap()
}

pub(super) fn idempotency_key(value: &str) -> IdempotencyKey {
    IdempotencyKey::new(value).unwrap()
}

pub(super) fn lease_ttl() -> DurationMs {
    DurationMs::from_millis(60_000)
}

pub(super) fn definition() -> CeremonyDefinition {
    let step = CeremonyStep::new(
        step_id(),
        StateId::new("COLLECTING_VOICES").unwrap(),
        StepHandlerKind::new("multiagent_round").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::new(StepAttempt::new(2).unwrap(), DurationMs::ZERO),
        None,
    );
    definition_with_step(step)
}

pub(super) fn child_spawning_definition() -> CeremonyDefinition {
    let spawn = CeremonyChildSpawn::new(
        vec![CeremonyChildSpec::new(
            CeremonyName::new("review_child").unwrap(),
            CeremonyVersion::v1(),
            BTreeMap::new(),
        )],
        MaxChildren::new(1).unwrap(),
        MaxChildDepth::new(2).unwrap(),
    )
    .unwrap();
    let step = CeremonyStep::new(
        step_id(),
        StateId::new("COLLECTING_VOICES").unwrap(),
        StepHandlerKind::new("must_not_run").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::new(StepAttempt::new(2).unwrap(), DurationMs::ZERO),
        None,
    )
    .with_spawn(spawn);
    definition_with_step(step)
}

/// Published definition targeted by [`child_spawning_definition`].
pub(super) fn review_child_definition() -> CeremonyDefinition {
    let state = StateId::new("REVIEWING").unwrap();
    let done = StateId::new("DONE").unwrap();
    let step = CeremonyStep::new(
        StepId::new("review").unwrap(),
        state.clone(),
        StepHandlerKind::new("multiagent_round").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::single_attempt(),
        None,
    );
    let guard = CeremonyGuard::new(
        GuardName::new("review_complete").unwrap(),
        GuardCondition::StepStatus {
            step_id: step.id().clone(),
            status: StepStatus::Completed,
        },
    );
    let transition = CeremonyTransition::new(
        state.clone(),
        done.clone(),
        TransitionTrigger::new("finish_review").unwrap(),
        vec![guard.name().clone()],
    )
    .unwrap();
    let role = CeremonyRole::new(
        RoleId::new("REVIEWER").unwrap(),
        vec![
            RoleAction::step(step.id().clone()),
            RoleAction::transition(transition.trigger().clone()),
        ],
    )
    .unwrap();
    CeremonyDefinition::new(
        CeremonyName::new("review_child").unwrap(),
        CeremonyVersion::v1(),
        None,
        Vec::new(),
        Vec::new(),
        vec![CeremonyState::initial(state), CeremonyState::terminal(done)],
        vec![transition],
        vec![step],
        vec![guard],
        vec![role],
    )
    .unwrap()
}

pub(super) fn context_writing_definition() -> CeremonyDefinition {
    let step = CeremonyStep::new(
        step_id(),
        StateId::new("COLLECTING_VOICES").unwrap(),
        StepHandlerKind::new("multiagent_round").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::new(StepAttempt::new(2).unwrap(), DurationMs::ZERO),
        None,
    )
    .with_context_writes(ContextWrites::new(BTreeMap::from([(
        ContextKey::new("summary").unwrap(),
        StepOutputField::new("summary").unwrap(),
    )])));
    definition_with_step(step)
}

pub(super) fn repeating_definition(max_iterations: u32) -> CeremonyDefinition {
    let step = CeremonyStep::new(
        step_id(),
        StateId::new("COLLECTING_VOICES").unwrap(),
        StepHandlerKind::new("multiagent_round").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::new(StepAttempt::new(2).unwrap(), DurationMs::ZERO),
        None,
    )
    .with_repeat_policy(StepRepeatPolicy::new(
        RepeatUntilCondition::output_field_equals(
            StepOutputField::new("ready").unwrap(),
            json!(true),
        ),
        StepIteration::new(max_iterations).unwrap(),
    ));
    definition_with_step(step)
}

pub(super) fn state_repeating_definition(max_iterations: u32) -> CeremonyDefinition {
    let state_id = StateId::new("REVIEWING").unwrap();
    let open = CeremonyStep::new(
        StepId::new("open").unwrap(),
        state_id.clone(),
        StepHandlerKind::new("multiagent_round").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::single_attempt(),
        None,
    );
    let check = CeremonyStep::new(
        StepId::new("check").unwrap(),
        state_id.clone(),
        StepHandlerKind::new("multiagent_round").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::single_attempt(),
        None,
    );
    let guard = CeremonyGuard::new(
        GuardName::new("review_done").unwrap(),
        GuardCondition::AllStepsCompleted,
    );
    let transition = CeremonyTransition::new(
        state_id.clone(),
        StateId::new("COMPLETED").unwrap(),
        trigger(),
        vec![guard.name().clone()],
    )
    .unwrap();
    let role = CeremonyRole::new(
        role_id(),
        vec![
            RoleAction::step(open.id().clone()),
            RoleAction::step(check.id().clone()),
            RoleAction::transition(transition.trigger().clone()),
        ],
    )
    .unwrap();
    CeremonyDefinition::new(
        CeremonyName::new("state_repeating_meeting").unwrap(),
        version(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(state_id).with_repeat_policy(StateRepeatPolicy::new(
                StateIteration::new(max_iterations).unwrap(),
                StateRepeatUntilCondition::new(
                    check.id().clone(),
                    StepOutputField::new("ready").unwrap(),
                    json!(true),
                ),
            )),
            CeremonyState::terminal(StateId::new("COMPLETED").unwrap()),
        ],
        vec![transition],
        vec![open, check],
        vec![guard],
        vec![role],
    )
    .unwrap()
}

pub(super) fn nested_repeating_definition() -> CeremonyDefinition {
    let state_id = StateId::new("REVIEWING").unwrap();
    let check = CeremonyStep::new(
        step_id(),
        state_id.clone(),
        StepHandlerKind::new("multiagent_round").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::single_attempt(),
        None,
    )
    .with_repeat_policy(StepRepeatPolicy::new(
        RepeatUntilCondition::output_field_equals(
            StepOutputField::new("ready").unwrap(),
            json!(true),
        ),
        StepIteration::FIRST,
    ));
    let guard = CeremonyGuard::new(
        GuardName::new("review_done").unwrap(),
        GuardCondition::StepStatus {
            step_id: check.id().clone(),
            status: StepStatus::Completed,
        },
    );
    let transition = CeremonyTransition::new(
        state_id.clone(),
        StateId::new("COMPLETED").unwrap(),
        trigger(),
        vec![guard.name().clone()],
    )
    .unwrap();
    let role = CeremonyRole::new(
        role_id(),
        vec![
            RoleAction::step(check.id().clone()),
            RoleAction::transition(transition.trigger().clone()),
        ],
    )
    .unwrap();
    CeremonyDefinition::new(
        CeremonyName::new("nested_repeating_meeting").unwrap(),
        version(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(state_id).with_repeat_policy(StateRepeatPolicy::new(
                StateIteration::new(3).unwrap(),
                StateRepeatUntilCondition::new(
                    check.id().clone(),
                    StepOutputField::new("state_ready").unwrap(),
                    json!(true),
                ),
            )),
            CeremonyState::terminal(StateId::new("COMPLETED").unwrap()),
        ],
        vec![transition],
        vec![check],
        vec![guard],
        vec![role],
    )
    .unwrap()
}

pub(super) fn nested_multi_iteration_definition() -> CeremonyDefinition {
    let state_id = StateId::new("REVIEWING").unwrap();
    let check = CeremonyStep::new(
        step_id(),
        state_id.clone(),
        StepHandlerKind::new("multiagent_round").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::single_attempt(),
        None,
    )
    .with_repeat_policy(StepRepeatPolicy::new(
        RepeatUntilCondition::output_field_equals(
            StepOutputField::new("ready").unwrap(),
            json!(true),
        ),
        StepIteration::new(2).unwrap(),
    ));
    let guard = CeremonyGuard::new(
        GuardName::new("review_done").unwrap(),
        GuardCondition::StepStatus {
            step_id: check.id().clone(),
            status: StepStatus::Completed,
        },
    );
    let transition = CeremonyTransition::new(
        state_id.clone(),
        StateId::new("COMPLETED").unwrap(),
        trigger(),
        vec![guard.name().clone()],
    )
    .unwrap();
    let role = CeremonyRole::new(
        role_id(),
        vec![
            RoleAction::step(check.id().clone()),
            RoleAction::transition(transition.trigger().clone()),
        ],
    )
    .unwrap();
    CeremonyDefinition::new(
        CeremonyName::new("nested_multi_iteration_meeting").unwrap(),
        version(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(state_id).with_repeat_policy(StateRepeatPolicy::new(
                StateIteration::new(2).unwrap(),
                StateRepeatUntilCondition::new(
                    check.id().clone(),
                    StepOutputField::new("accepted").unwrap(),
                    json!(true),
                ),
            )),
            CeremonyState::terminal(StateId::new("COMPLETED").unwrap()),
        ],
        vec![transition],
        vec![check],
        vec![guard],
        vec![role],
    )
    .unwrap()
}

pub(super) fn repeating_approval_definition(max_iterations: u32) -> CeremonyDefinition {
    let step = CeremonyStep::new(
        step_id(),
        StateId::new("COLLECTING_VOICES").unwrap(),
        StepHandlerKind::new("multiagent_round").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::new(StepAttempt::new(2).unwrap(), DurationMs::ZERO),
        None,
    )
    .with_repeat_policy(StepRepeatPolicy::new(
        RepeatUntilCondition::output_field_equals(
            StepOutputField::new("ready").unwrap(),
            json!(true),
        ),
        StepIteration::new(max_iterations).unwrap(),
    ));
    let guard = CeremonyGuard::new(
        GuardName::new("human_approved").unwrap(),
        GuardCondition::HumanApproval,
    );
    let transition = CeremonyTransition::new(
        StateId::new("COLLECTING_VOICES").unwrap(),
        StateId::new("COMPLETED").unwrap(),
        trigger(),
        vec![guard.name().clone()],
    )
    .unwrap();
    let role = CeremonyRole::new(
        role_id(),
        vec![
            RoleAction::step(step.id().clone()),
            RoleAction::transition(transition.trigger().clone()),
        ],
    )
    .unwrap();

    CeremonyDefinition::new(
        CeremonyName::new("repeating_approval_ceremony").unwrap(),
        version(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(StateId::new("COLLECTING_VOICES").unwrap()),
            CeremonyState::terminal(StateId::new("COMPLETED").unwrap()),
        ],
        vec![transition],
        vec![step],
        vec![guard],
        vec![role],
    )
    .unwrap()
}

fn definition_with_step(step: CeremonyStep) -> CeremonyDefinition {
    let guard = CeremonyGuard::new(
        GuardName::new("roundtable_completed").unwrap(),
        GuardCondition::StepStatus {
            step_id: step.id().clone(),
            status: StepStatus::Completed,
        },
    );
    let transition = CeremonyTransition::new(
        StateId::new("COLLECTING_VOICES").unwrap(),
        StateId::new("COMPLETED").unwrap(),
        trigger(),
        vec![guard.name().clone()],
    )
    .unwrap();
    let role = CeremonyRole::new(
        role_id(),
        vec![
            RoleAction::step(step.id().clone()),
            RoleAction::transition(transition.trigger().clone()),
            RoleAction::request_intervention(),
        ],
    )
    .unwrap();
    let respondent = CeremonyRole::new(
        respondent_role_id(),
        vec![RoleAction::respond_to_intervention()],
    )
    .unwrap();

    CeremonyDefinition::new(
        definition_name(),
        version(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(StateId::new("COLLECTING_VOICES").unwrap()),
            CeremonyState::terminal(StateId::new("COMPLETED").unwrap()),
        ],
        vec![transition],
        vec![step],
        vec![guard],
        vec![role, respondent],
    )
    .unwrap()
}

/// A two-step linear ceremony (`open` in OPENING, `respond` in
/// RESPONDING) used to prove that step outputs thread forward into the
/// transcript the next step receives.
pub(super) fn two_step_definition() -> CeremonyDefinition {
    let open = CeremonyStep::new(
        StepId::new("open").unwrap(),
        StateId::new("OPENING").unwrap(),
        StepHandlerKind::new("multiagent_round").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::single_attempt(),
        None,
    );
    let respond = CeremonyStep::new(
        StepId::new("respond").unwrap(),
        StateId::new("RESPONDING").unwrap(),
        StepHandlerKind::new("multiagent_round").unwrap(),
        StepHandlerConfig::empty(),
        RetryPolicy::single_attempt(),
        None,
    );
    let open_done = CeremonyGuard::new(
        GuardName::new("open_done").unwrap(),
        GuardCondition::StepStatus {
            step_id: open.id().clone(),
            status: StepStatus::Completed,
        },
    );
    let respond_done = CeremonyGuard::new(
        GuardName::new("respond_done").unwrap(),
        GuardCondition::StepStatus {
            step_id: respond.id().clone(),
            status: StepStatus::Completed,
        },
    );
    let opened = CeremonyTransition::new(
        StateId::new("OPENING").unwrap(),
        StateId::new("RESPONDING").unwrap(),
        TransitionTrigger::new("opened").unwrap(),
        vec![open_done.name().clone()],
    )
    .unwrap();
    let responded = CeremonyTransition::new(
        StateId::new("RESPONDING").unwrap(),
        StateId::new("CLOSED").unwrap(),
        TransitionTrigger::new("responded").unwrap(),
        vec![respond_done.name().clone()],
    )
    .unwrap();
    let role = CeremonyRole::new(
        role_id(),
        vec![
            RoleAction::step(open.id().clone()),
            RoleAction::step(respond.id().clone()),
            RoleAction::transition(opened.trigger().clone()),
            RoleAction::transition(responded.trigger().clone()),
        ],
    )
    .unwrap();

    CeremonyDefinition::new(
        CeremonyName::new("two_step_meeting").unwrap(),
        version(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(StateId::new("OPENING").unwrap()),
            CeremonyState::intermediate(StateId::new("RESPONDING").unwrap()),
            CeremonyState::terminal(StateId::new("CLOSED").unwrap()),
        ],
        vec![opened, responded],
        vec![open, respond],
        vec![open_done, respond_done],
        vec![role],
    )
    .unwrap()
}

pub(super) fn approval_definition() -> CeremonyDefinition {
    let guard_name = GuardName::new("human_approved").unwrap();
    let guard = CeremonyGuard::new(guard_name.clone(), GuardCondition::HumanApproval);
    let transition = CeremonyTransition::new(
        StateId::new("STARTED").unwrap(),
        StateId::new("APPROVED").unwrap(),
        TransitionTrigger::new("approve").unwrap(),
        vec![guard_name],
    )
    .unwrap();
    let role = CeremonyRole::new(
        role_id(),
        vec![RoleAction::transition(transition.trigger().clone())],
    )
    .unwrap();

    CeremonyDefinition::new(
        approval_definition_name(),
        version(),
        None,
        Vec::new(),
        Vec::new(),
        vec![
            CeremonyState::initial(StateId::new("STARTED").unwrap()),
            CeremonyState::terminal(StateId::new("APPROVED").unwrap()),
        ],
        vec![transition],
        Vec::new(),
        vec![guard],
        vec![role],
    )
    .unwrap()
}

pub(super) fn started_instance(definition: &CeremonyDefinition) -> CeremonyInstance {
    CeremonyInstance::start(ceremony_id(), definition, CeremonyContext::empty(), now())
        .expect("required ceremony inputs")
}

/// The published catalogue. Empty by default, because most tests run
/// an unbound session; seed it when the point of the test is a session
/// that is bound to what it runs.
impl PublicationsFake {
    pub(super) async fn seed(&self, definition: CeremonyDefinition) -> PublishedCeremonyDefinition {
        let sealed = PublishedCeremonyDefinition::seal(definition).unwrap();
        self.published.write().await.insert(
            (
                sealed.name().as_str().to_owned(),
                sealed.version().as_str().to_owned(),
            ),
            sealed.clone(),
        );
        sealed
    }
}

#[async_trait]
impl CeremonyDefinitionPublicationPort for PublicationsFake {
    async fn publish(
        &self,
        definition: PublishedCeremonyDefinition,
    ) -> Result<PublicationOutcome, DomainError> {
        let key = (
            definition.name().as_str().to_owned(),
            definition.version().as_str().to_owned(),
        );
        let mut published = self.published.write().await;
        if let Some(occupant) = published.get(&key) {
            return Ok(PublicationOutcome::VersionOccupied {
                published: occupant.digest(),
                offered: definition.digest(),
            });
        }
        published.insert(key, definition.clone());
        Ok(PublicationOutcome::Published(definition))
    }

    async fn published(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<Option<PublishedCeremonyDefinition>, DomainError> {
        Ok(self
            .published
            .read()
            .await
            .get(&(name.as_str().to_owned(), version.as_str().to_owned()))
            .cloned())
    }

    async fn catalogue(&self) -> Result<Vec<PublishedCeremonyDefinition>, DomainError> {
        Ok(self.published.read().await.values().cloned().collect())
    }
}

/// The resolver every use case that advances a session now takes. Most
/// tests want the plain repository behind it and nothing published.
pub(super) fn definition_resolver(
    definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
) -> Arc<ResolveCeremonyDefinitionUseCase> {
    resolver_with(definitions, Arc::new(PublicationsFake::default()))
}

pub(super) fn resolver_with(
    definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
    publications: Arc<dyn CeremonyDefinitionPublicationPort>,
) -> Arc<ResolveCeremonyDefinitionUseCase> {
    Arc::new(ResolveCeremonyDefinitionUseCase::new(
        definitions,
        publications,
    ))
}

/// Memory that keeps what it was handed, so a test can see what the
/// engine chose to remember — and why it said one thing led to
/// another.
///
/// Not a stand-in for a memory backend. What is worth checking here is the
/// engine's judgement, and that is the same whatever backend receives
/// it.
impl RecordingMemory {
    pub(super) async fn entries(&self) -> Vec<MemoryEntry> {
        self.written
            .read()
            .await
            .iter()
            .flat_map(|(_, write, _)| write.entries().to_vec())
            .collect()
    }

    pub(super) async fn relations(&self) -> Vec<MemoryRelation> {
        self.written
            .read()
            .await
            .iter()
            .flat_map(|(_, write, _)| write.relations().to_vec())
            .collect()
    }
}

#[async_trait]
impl MemoryWriterPort for RecordingMemory {
    async fn remember(
        &self,
        scope: &MemoryScope,
        write: MemoryWrite,
        idempotency_key: &str,
    ) -> Result<MemoryWriteOutcome, DomainError> {
        self.written
            .write()
            .await
            .push((scope.clone(), write, idempotency_key.to_owned()));
        Ok(MemoryWriteOutcome::Remembered)
    }

    fn capabilities(&self) -> MemoryCapabilities {
        MemoryCapabilities::none()
            .with(MemoryCapability::Remembering)
            .with(MemoryCapability::KeepingReasons)
    }
}

/// What was written comes back, which is what a session recalling its
/// own scope has to see.
#[async_trait]
impl MemoryReaderPort for RecordingMemory {
    async fn recall(&self, scope: &MemoryScope) -> Result<MemoryRecollection, DomainError> {
        Ok(MemoryRecollection::of(
            self.written
                .read()
                .await
                .iter()
                .filter(|(written_to, _, _)| written_to == scope)
                .flat_map(|(_, write, _)| write.entries().to_vec())
                .collect(),
        ))
    }

    async fn as_known_at(
        &self,
        _scope: &MemoryScope,
        _moment: MemoryMoment,
    ) -> Result<MemoryRecollection, DomainError> {
        Ok(MemoryRecollection::Unsupported)
    }

    async fn follow(
        &self,
        _scope: &MemoryScope,
        _from: &MemoryEntryId,
        _to: &MemoryEntryId,
    ) -> Result<MemoryRecollection, DomainError> {
        Ok(MemoryRecollection::Unsupported)
    }

    fn capabilities(&self) -> MemoryCapabilities {
        MemoryCapabilities::none()
            .with(MemoryCapability::Remembering)
            .with(MemoryCapability::Recalling)
    }
}

#[async_trait]
impl MemoryReaderPort for MemoryThatIsOut {
    async fn recall(&self, _scope: &MemoryScope) -> Result<MemoryRecollection, DomainError> {
        Err(DomainError::InvalidDocument {
            reason: "the memory backend did not answer".to_owned(),
        })
    }

    async fn as_known_at(
        &self,
        _scope: &MemoryScope,
        _moment: MemoryMoment,
    ) -> Result<MemoryRecollection, DomainError> {
        Ok(MemoryRecollection::Unsupported)
    }

    async fn follow(
        &self,
        _scope: &MemoryScope,
        _from: &MemoryEntryId,
        _to: &MemoryEntryId,
    ) -> Result<MemoryRecollection, DomainError> {
        Ok(MemoryRecollection::Unsupported)
    }

    fn capabilities(&self) -> MemoryCapabilities {
        MemoryCapabilities::none()
    }
}

pub(super) fn recording_memory() -> Arc<RecordingMemory> {
    Arc::new(RecordingMemory::default())
}

/// A stream whose sessions are recorded into `memory`, the way a
/// composition root wires one.
///
/// The recorder is a subscriber of the stream, not something a use
/// case holds, so this is the whole of turning memory on in a test:
/// the use cases under it are built exactly as they are in production.
pub(super) fn remembering_stream(
    store: Arc<EventStoreFake>,
    memory: Arc<RecordingMemory>,
) -> Arc<SessionStream> {
    stream_watched_by(
        store.clone(),
        Arc::new(SessionMemoryRecorder::new(memory, store)),
    )
}

/// Something a start use case can read, for the many tests that do not
/// care what is in it.
pub(super) fn a_memory() -> Arc<dyn MemoryReaderPort> {
    recording_memory()
}

/// One decision, already remembered, as an earlier session would have
/// left it.
pub(super) fn remembered_decision(summary: &str) -> MemoryEntry {
    MemoryEntry::new(
        MemoryEntryId::new(summary).expect("a valid entry id"),
        MemoryEntryKind::Decision,
        summary,
        MemoryProvenance::new(
            CeremonyId::new("earlier-session").expect("a valid ceremony id"),
            None,
            datetime!(2026-07-28 09:00:00 UTC),
        ),
        Attributes::empty(),
    )
    .expect("a valid entry")
}

/// The store every session test reads from and appends to.
///
/// `save` seeds a session the way the old repository fake did: the
/// stream gets the opening record the instance would have produced,
/// and the instance as handed in rides in a snapshot at version one.
/// For a freshly started instance that snapshot is exactly the fold;
/// for one a test mutated before seeding, it is the state the test
/// wants the use case to find, which the real store would only hold
/// after the events that produced it. Nothing seeded lands in `facts`,
/// so a test that counts what a use case sealed counts only that.
impl EventStoreFake {
    pub(super) async fn save(&self, instance: &CeremonyInstance) -> Result<(), DomainError> {
        let started = CeremonyEvent::CeremonyInstanceStarted(
            made_core::entities::ceremony_events::CeremonyInstanceStarted {
                ceremony_id: instance.id().clone(),
                definition_name: instance.definition_name().clone(),
                definition_version: instance.definition_version().clone(),
                initial_state: instance.current_state().clone(),
                step_ids: instance.step_records().keys().cloned().collect(),
                context: instance.context().clone(),
                bound_definition: instance.bound_definition(),
                lineage: instance.lineage().cloned(),
                succession: instance.succession().cloned().map(Box::new),
                budget_account_id: instance.budget_account_id().cloned(),
                ceremony_deadline: instance.ceremony_deadline(),
                state_deadline: instance.state_deadline().cloned(),
                created_at: instance.created_at(),
            },
        );
        let mut opening = session_facts::fact(
            instance,
            started,
            session_facts::party("fixture", AuditActorKind::Service)?,
            instance.created_at(),
        )?;
        opening.correlation_id = Some(opening.event_id.clone());
        let outcome = self
            .append(instance.id(), StreamVersion::EMPTY, vec![opening])
            .await?;
        let Some(version) = outcome.appended_version() else {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_instance",
            });
        };
        // Seeded, not sealed: the fixture's opening is not something
        // the use case under test did.
        self.facts.write().await.clear();
        self.snapshots
            .write()
            .await
            .entry(instance.id().clone())
            .or_default()
            .insert(version, instance.clone());
        Ok(())
    }

    /// The session as the engine would load it now.
    pub(super) async fn saved(self: &Arc<Self>, id: &CeremonyId) -> CeremonyInstance {
        stream(self.clone()).load(id).await.unwrap().instance
    }

    /// Whether a stream was ever opened for this id.
    pub(super) async fn exists(&self, id: &CeremonyId) -> bool {
        !self.head(id).await.unwrap().is_empty()
    }

    /// Every fact appended through the port, in order.
    pub(super) async fn facts(&self) -> Vec<AuditFact> {
        self.facts.read().await.clone()
    }

    /// The sealed records of one stream, in order.
    pub(super) async fn records(&self, id: &CeremonyId) -> Vec<AuditRecord> {
        self.streams
            .read()
            .await
            .get(id)
            .cloned()
            .unwrap_or_default()
    }
}

fn version_of(records: &[AuditRecord]) -> StreamVersion {
    records.last().map_or(StreamVersion::EMPTY, |record| {
        StreamVersion::from_sequence(record.sequence())
    })
}

#[async_trait]
impl CeremonyEventStorePort for EventStoreFake {
    async fn append(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        let mut streams = self.streams.write().await;
        let existing = streams.entry(stream.clone()).or_default();
        let actual = version_of(existing);
        if actual != expected {
            return Ok(AppendOutcome::Conflict { expected, actual });
        }
        let sealed = seal_continuation(stream, existing, facts.clone(), None)?;
        let mut log = self.log.write().await;
        let first_position = GlobalPosition::new(u64::try_from(log.len()).unwrap() + 1).unwrap();
        for record in &sealed {
            log.push((stream.clone(), record.sequence()));
        }
        existing.extend(sealed.iter().cloned());
        self.facts.write().await.extend(facts);
        Ok(AppendOutcome::Appended {
            version: version_of(existing),
            records: sealed,
            first_position,
        })
    }

    async fn read(
        &self,
        stream: &CeremonyId,
        after: StreamVersion,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        Ok(self
            .streams
            .read()
            .await
            .get(stream)
            .map(|records| {
                records
                    .iter()
                    .filter(|record| record.sequence().value() > after.value())
                    .take(limit.value())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        let streams = self.streams.read().await;
        let log = self.log.read().await;
        Ok(log
            .iter()
            .enumerate()
            .map(|(index, entry)| (GlobalPosition::new(index as u64 + 1).unwrap(), entry))
            .filter(|(position, _)| *position >= from)
            .take(limit.value())
            .map(|(position, (stream, sequence))| PositionedRecord {
                position,
                record: streams[stream][usize::try_from(sequence.value()).unwrap() - 1].clone(),
            })
            .collect())
    }

    async fn head(&self, stream: &CeremonyId) -> Result<StreamVersion, DomainError> {
        Ok(self
            .streams
            .read()
            .await
            .get(stream)
            .map_or(StreamVersion::EMPTY, |records| version_of(records)))
    }

    async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError> {
        Ok(self
            .streams
            .read()
            .await
            .iter()
            .filter(|(_, records)| !records.is_empty())
            .map(|(id, _)| id.clone())
            .collect())
    }
}

#[async_trait]
impl CeremonySnapshotStorePort for EventStoreFake {
    async fn save(&self, snapshot: CeremonySnapshot) -> Result<(), DomainError> {
        self.snapshots
            .write()
            .await
            .entry(snapshot.instance.id().clone())
            .or_default()
            .insert(snapshot.version, snapshot.instance);
        Ok(())
    }

    async fn latest(&self, stream: &CeremonyId) -> Result<Option<CeremonySnapshot>, DomainError> {
        Ok(self
            .snapshots
            .read()
            .await
            .get(stream)
            .and_then(|versions| {
                versions
                    .iter()
                    .next_back()
                    .map(|(version, instance)| CeremonySnapshot {
                        version: *version,
                        instance: instance.clone(),
                    })
            }))
    }

    async fn forget(&self, stream: &CeremonyId) -> Result<(), DomainError> {
        self.snapshots.write().await.remove(stream);
        Ok(())
    }
}

/// The stream every session use case is built over.
pub(super) fn stream(store: Arc<EventStoreFake>) -> Arc<SessionStream> {
    Arc::new(SessionStream::new(
        store.clone(),
        store,
        Arc::new(NoopCeremonyEventSubscriber),
    ))
}

/// The same stream, with something projecting from it.
pub(super) fn stream_watched_by(
    store: Arc<EventStoreFake>,
    subscriber: Arc<dyn CeremonyEventSubscriberPort>,
) -> Arc<SessionStream> {
    Arc::new(SessionStream::new(store.clone(), store, subscriber))
}

/// A stream a test can look inside.
pub(super) fn stream_over(store: Arc<EventStoreFake>) -> (Arc<SessionStream>, Arc<EventStoreFake>) {
    (stream(store.clone()), store)
}

/// A stream whose first append is overtaken by another writer, and
/// whose later ones land.
pub(super) fn stream_conflicting_once(store: Arc<EventStoreFake>) -> Arc<SessionStream> {
    stream_conflicting_once_watched_by(store, Arc::new(NoopCeremonyEventSubscriber))
}

/// The same, with something projecting from it: what a subscriber is
/// told when an append is refused once and lands on the retry.
pub(super) fn stream_conflicting_once_watched_by(
    store: Arc<EventStoreFake>,
    subscriber: Arc<dyn CeremonyEventSubscriberPort>,
) -> Arc<SessionStream> {
    Arc::new(SessionStream::new(
        Arc::new(StoreThatConflictsOnce {
            inner: store.clone(),
            conflicted: std::sync::atomic::AtomicBool::new(false),
            overtaking_facts: Vec::new(),
            conflict_on: None,
        }),
        store,
        subscriber,
    ))
}

/// A stream whose first append loses to the supplied fact.
pub(super) fn stream_overtaken_once(
    store: Arc<EventStoreFake>,
    overtaking_fact: AuditFact,
) -> Arc<SessionStream> {
    Arc::new(SessionStream::new(
        Arc::new(StoreThatConflictsOnce {
            inner: store.clone(),
            conflicted: std::sync::atomic::AtomicBool::new(false),
            overtaking_facts: vec![overtaking_fact],
            conflict_on: None,
        }),
        store,
        Arc::new(NoopCeremonyEventSubscriber),
    ))
}

/// A stream whose first step claim loses to the supplied durable facts.
pub(super) fn stream_overtaken_on_step_claim(
    store: Arc<EventStoreFake>,
    overtaking_facts: Vec<AuditFact>,
) -> Arc<SessionStream> {
    Arc::new(SessionStream::new(
        Arc::new(StoreThatConflictsOnce {
            inner: store.clone(),
            conflicted: std::sync::atomic::AtomicBool::new(false),
            overtaking_facts,
            conflict_on: Some(AuditEventType::StepStarted),
        }),
        store,
        Arc::new(NoopCeremonyEventSubscriber),
    ))
}

/// A stream whose every append is overtaken by another writer.
pub(super) fn stream_losing_every_race(store: Arc<EventStoreFake>) -> Arc<SessionStream> {
    stream_losing_every_race_over(store).0
}

/// The same, with something projecting from it: an append that never
/// lands has nothing to tell it.
pub(super) fn stream_losing_every_race_watched_by(
    store: Arc<EventStoreFake>,
    subscriber: Arc<dyn CeremonyEventSubscriberPort>,
) -> Arc<SessionStream> {
    let losing = Arc::new(StoreThatLosesEveryRace {
        inner: store.clone(),
        appends: std::sync::atomic::AtomicUsize::new(0),
    });
    Arc::new(SessionStream::new(losing, store, subscriber))
}

/// The same, with the losing store in hand so a test can count how
/// many times the use case tried before giving up.
pub(super) fn stream_losing_every_race_over(
    store: Arc<EventStoreFake>,
) -> (Arc<SessionStream>, Arc<StoreThatLosesEveryRace>) {
    let losing = Arc::new(StoreThatLosesEveryRace {
        inner: store.clone(),
        appends: std::sync::atomic::AtomicUsize::new(0),
    });
    (
        Arc::new(SessionStream::new(
            losing.clone(),
            store,
            Arc::new(NoopCeremonyEventSubscriber),
        )),
        losing,
    )
}

impl StoreThatLosesEveryRace {
    pub(super) fn appends(&self) -> usize {
        self.appends.load(std::sync::atomic::Ordering::SeqCst)
    }
}

fn overtaken(expected: StreamVersion) -> AppendOutcome {
    AppendOutcome::Conflict {
        expected,
        actual: expected.next(),
    }
}

#[async_trait]
impl CeremonyEventStorePort for StoreThatConflictsOnce {
    async fn append(
        &self,
        stream: &CeremonyId,
        expected: StreamVersion,
        facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        let matches = self.conflict_on.is_none_or(|event_type| {
            facts
                .iter()
                .any(|fact| fact.event.event_type() == event_type)
        });
        if matches
            && !self
                .conflicted
                .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            if !self.overtaking_facts.is_empty() {
                let outcome = self
                    .inner
                    .append(stream, expected, self.overtaking_facts.clone())
                    .await?;
                return Ok(AppendOutcome::Conflict {
                    expected,
                    actual: outcome
                        .appended_version()
                        .expect("overtaking facts must land"),
                });
            }
            return Ok(overtaken(expected));
        }
        self.inner.append(stream, expected, facts).await
    }

    async fn read(
        &self,
        stream: &CeremonyId,
        after: StreamVersion,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        self.inner.read(stream, after, limit).await
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        self.inner.read_all(from, limit).await
    }

    async fn head(&self, stream: &CeremonyId) -> Result<StreamVersion, DomainError> {
        self.inner.head(stream).await
    }

    async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError> {
        self.inner.streams().await
    }
}

#[async_trait]
impl CeremonyEventStorePort for StoreThatLosesEveryRace {
    async fn append(
        &self,
        _stream: &CeremonyId,
        expected: StreamVersion,
        _facts: Vec<AuditFact>,
    ) -> Result<AppendOutcome, DomainError> {
        self.appends
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(overtaken(expected))
    }

    async fn read(
        &self,
        stream: &CeremonyId,
        after: StreamVersion,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<AuditRecord>, DomainError> {
        self.inner.read(stream, after, limit).await
    }

    async fn read_all(
        &self,
        from: GlobalPosition,
        limit: CeremonyEventPageLimit,
    ) -> Result<Vec<PositionedRecord>, DomainError> {
        self.inner.read_all(from, limit).await
    }

    async fn head(&self, stream: &CeremonyId) -> Result<StreamVersion, DomainError> {
        self.inner.head(stream).await
    }

    async fn streams(&self) -> Result<Vec<CeremonyId>, DomainError> {
        self.inner.streams().await
    }
}
