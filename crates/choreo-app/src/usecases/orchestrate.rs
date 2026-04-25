//! [`OrchestrateUseCase`] — deliberate and then hand the winning
//! proposal to the configured [`ExecutorPort`].
//!
//! Emits a `TaskDispatchedEvent` when dispatching to the executor and
//! either a `TaskCompletedEvent` or `TaskFailedEvent` at the end, so
//! downstream systems can observe the full lifecycle.

use std::sync::Arc;

use choreo_core::entities::{Deliberation, Proposal, Task};
use choreo_core::error::DomainError;
use choreo_core::events::{
    EventEnvelope, TaskCompletedEvent, TaskDispatchedEvent, TaskFailedEvent,
};
use choreo_core::ports::{ClockPort, ExecutionOutcome, ExecutorPort, MessagingPort};
use choreo_core::value_objects::{Attributes, EventId};
use time::OffsetDateTime;
use tracing::{error, info, warn};
use uuid::Uuid;

use super::deliberate::DeliberateUseCase;

#[derive(Debug, Clone)]
pub struct OrchestrateOutput {
    pub deliberation: Deliberation,
    pub winner: Proposal,
    pub execution: ExecutionOutcome,
}

pub struct OrchestrateUseCase {
    deliberate: Arc<DeliberateUseCase>,
    executor: Arc<dyn ExecutorPort>,
    messaging: Arc<dyn MessagingPort>,
    clock: Arc<dyn ClockPort>,
    statistics: Arc<dyn choreo_core::ports::StatisticsPort>,
    source: String,
}

impl std::fmt::Debug for OrchestrateUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrchestrateUseCase")
            .field("source", &self.source)
            .finish()
    }
}

impl OrchestrateUseCase {
    #[must_use]
    pub fn new(
        deliberate: Arc<DeliberateUseCase>,
        executor: Arc<dyn ExecutorPort>,
        messaging: Arc<dyn MessagingPort>,
        clock: Arc<dyn ClockPort>,
        statistics: Arc<dyn choreo_core::ports::StatisticsPort>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            deliberate,
            executor,
            messaging,
            clock,
            statistics,
            source: source.into(),
        }
    }

    #[tracing::instrument(
        name = "orchestrate",
        skip_all,
        fields(
            task_id = %task.id(),
            specialty = %task.specialty(),
        )
    )]
    pub async fn execute(
        &self,
        task: Task,
        execution_options: Attributes,
    ) -> Result<OrchestrateOutput, DomainError> {
        let task_id = task.id().clone();
        let specialty = task.specialty().clone();

        let out = match self.deliberate.execute(task).await {
            Ok(out) => out,
            Err(err) => {
                let event = TaskFailedEvent::new(
                    self.envelope(self.clock.now())?,
                    task_id.clone(),
                    specialty.clone(),
                    deliberation_error_kind(&err),
                    err.to_string(),
                )?;
                self.messaging.publish_task_failed(&event).await?;
                error!(
                    task_id = task_id.as_str(),
                    error = %err,
                    "orchestration failed during deliberation"
                );
                return Err(err);
            }
        };
        let winner = out
            .deliberation
            .proposals()
            .get(&out.winner_proposal_id)
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "winner proposal",
            })?;

        self.messaging
            .publish_task_dispatched(&TaskDispatchedEvent::new(
                self.envelope(self.clock.now())?,
                task_id.clone(),
                specialty.clone(),
                None,
            ))
            .await?;

        match self.executor.execute(&winner, &execution_options).await {
            Ok(execution) => {
                // The deliberation's own duration already went into
                // the stats through `DeliberateUseCase`; here we
                // record the orchestration end-to-end duration,
                // which for the noop executor is just the executor
                // call's reported duration.
                self.statistics
                    .record_orchestration(execution.duration)
                    .await?;

                if execution.succeeded {
                    self.messaging
                        .publish_task_completed(&TaskCompletedEvent::new(
                            self.envelope(self.clock.now())?,
                            task_id.clone(),
                            specialty.clone(),
                            Some(winner.author().clone()),
                            execution.duration,
                        ))
                        .await?;
                    info!(
                        task_id = task_id.as_str(),
                        execution_id = execution.execution_id.as_str(),
                        "orchestration completed"
                    );
                } else {
                    let event = TaskFailedEvent::new(
                        self.envelope(self.clock.now())?,
                        task_id.clone(),
                        specialty.clone(),
                        "executor.unsuccessful",
                        "executor reported unsuccessful outcome",
                    )?;
                    self.messaging.publish_task_failed(&event).await?;
                    warn!(
                        task_id = task_id.as_str(),
                        execution_id = execution.execution_id.as_str(),
                        "orchestration finished with unsuccessful execution outcome"
                    );
                }
                Ok(OrchestrateOutput {
                    deliberation: out.deliberation,
                    winner,
                    execution,
                })
            }
            Err(err) => {
                let event = TaskFailedEvent::new(
                    self.envelope(self.clock.now())?,
                    task_id.clone(),
                    specialty,
                    "executor.error",
                    err.to_string(),
                )?;
                self.messaging.publish_task_failed(&event).await?;
                error!(task_id = task_id.as_str(), error = %err, "orchestration failed");
                Err(err)
            }
        }
    }

    fn envelope(&self, emitted_at: OffsetDateTime) -> Result<EventEnvelope, DomainError> {
        EventEnvelope::new(
            EventId::new(Uuid::new_v4().to_string())?,
            emitted_at,
            self.source.clone(),
            None,
        )
    }
}

fn deliberation_error_kind(err: &DomainError) -> &'static str {
    match err {
        DomainError::NoValidProposal { .. } => "deliberation.no_valid_proposal",
        _ => "deliberation.error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use choreo_core::entities::{
        Council, Deliberation, DeliberationPhase, Task, TaskConstraints, ValidatorReport,
    };
    use choreo_core::events::{DeliberationCompletedEvent, PhaseChangedEvent, TaskDispatchedEvent};
    use choreo_core::ports::{
        AgentPort, AgentResolverPort, CouncilRegistryPort, Critique, DeliberationRepositoryPort,
        DraftRequest, Revision, ScoringPort, ValidatorPort,
    };
    use choreo_core::value_objects::{
        AgentId, Attributes, CouncilId, DurationMs, OutputContract, OutputFieldRule, Rounds,
        Rubric, Score, Specialty, TaskDescription, TaskId,
    };
    use time::macros::datetime;

    struct FrozenClock {
        now: OffsetDateTime,
    }

    impl ClockPort for FrozenClock {
        fn now(&self) -> OffsetDateTime {
            self.now
        }
    }

    #[derive(Debug)]
    struct StubAgent {
        id: AgentId,
        specialty: Specialty,
        draft: String,
    }

    impl StubAgent {
        fn new(id: &str, specialty: &Specialty, draft: &str) -> Arc<Self> {
            Arc::new(Self {
                id: AgentId::new(id).unwrap(),
                specialty: specialty.clone(),
                draft: draft.to_owned(),
            })
        }
    }

    #[async_trait]
    impl AgentPort for StubAgent {
        fn id(&self) -> &AgentId {
            &self.id
        }

        fn specialty(&self) -> &Specialty {
            &self.specialty
        }

        async fn generate(&self, _request: DraftRequest) -> Result<Revision, DomainError> {
            Ok(Revision {
                content: self.draft.clone(),
            })
        }

        async fn critique(
            &self,
            _peer_content: &str,
            _constraints: &TaskConstraints,
        ) -> Result<Critique, DomainError> {
            Ok(Critique {
                feedback: "ok".to_owned(),
            })
        }

        async fn revise(
            &self,
            own_content: &str,
            _critique: &Critique,
        ) -> Result<Revision, DomainError> {
            Ok(Revision {
                content: own_content.to_owned(),
            })
        }
    }

    struct StubResolver {
        agent: Arc<dyn AgentPort>,
    }

    #[async_trait]
    impl AgentResolverPort for StubResolver {
        async fn resolve(&self, id: &AgentId) -> Result<Arc<dyn AgentPort>, DomainError> {
            if id == self.agent.id() {
                Ok(self.agent.clone())
            } else {
                Err(DomainError::NotFound { what: "agent" })
            }
        }
    }

    struct FixedRegistry {
        council: Council,
    }

    #[async_trait]
    impl CouncilRegistryPort for FixedRegistry {
        async fn register(&self, _council: Council) -> Result<(), DomainError> {
            unimplemented!()
        }

        async fn replace(&self, _council: Council) -> Result<(), DomainError> {
            unimplemented!()
        }

        async fn get(&self, specialty: &Specialty) -> Result<Council, DomainError> {
            if specialty == self.council.specialty() {
                Ok(self.council.clone())
            } else {
                Err(DomainError::NotFound { what: "council" })
            }
        }

        async fn list(&self) -> Result<Vec<Council>, DomainError> {
            Ok(vec![self.council.clone()])
        }

        async fn delete(&self, _specialty: &Specialty) -> Result<(), DomainError> {
            unimplemented!()
        }

        async fn contains(&self, specialty: &Specialty) -> Result<bool, DomainError> {
            Ok(specialty == self.council.specialty())
        }
    }

    #[derive(Default)]
    struct InMemoryRepo {
        saved: Mutex<Vec<Deliberation>>,
    }

    #[async_trait]
    impl DeliberationRepositoryPort for InMemoryRepo {
        async fn save(&self, deliberation: &Deliberation) -> Result<(), DomainError> {
            self.saved.lock().unwrap().push(deliberation.clone());
            Ok(())
        }

        async fn get(&self, task_id: &TaskId) -> Result<Deliberation, DomainError> {
            self.saved
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|deliberation| deliberation.task_id() == task_id)
                .cloned()
                .ok_or(DomainError::NotFound {
                    what: "deliberation",
                })
        }

        async fn exists(&self, task_id: &TaskId) -> Result<bool, DomainError> {
            Ok(self
                .saved
                .lock()
                .unwrap()
                .iter()
                .any(|deliberation| deliberation.task_id() == task_id))
        }
    }

    #[derive(Default)]
    struct RecordingBus {
        dispatched: Mutex<Vec<TaskDispatchedEvent>>,
        completed: Mutex<Vec<TaskCompletedEvent>>,
        failed: Mutex<Vec<TaskFailedEvent>>,
        deliberation_completed: Mutex<Vec<DeliberationCompletedEvent>>,
    }

    #[async_trait]
    impl MessagingPort for RecordingBus {
        async fn publish_task_dispatched(
            &self,
            event: &TaskDispatchedEvent,
        ) -> Result<(), DomainError> {
            self.dispatched.lock().unwrap().push(event.clone());
            Ok(())
        }

        async fn publish_task_completed(
            &self,
            event: &TaskCompletedEvent,
        ) -> Result<(), DomainError> {
            self.completed.lock().unwrap().push(event.clone());
            Ok(())
        }

        async fn publish_task_failed(&self, event: &TaskFailedEvent) -> Result<(), DomainError> {
            self.failed.lock().unwrap().push(event.clone());
            Ok(())
        }

        async fn publish_deliberation_completed(
            &self,
            event: &DeliberationCompletedEvent,
        ) -> Result<(), DomainError> {
            self.deliberation_completed
                .lock()
                .unwrap()
                .push(event.clone());
            Ok(())
        }

        async fn publish_phase_changed(
            &self,
            _event: &PhaseChangedEvent,
        ) -> Result<(), DomainError> {
            Ok(())
        }
    }

    struct PassValidator;

    #[async_trait]
    impl ValidatorPort for PassValidator {
        fn kind(&self) -> &'static str {
            "pass"
        }

        async fn validate(
            &self,
            _content: &str,
            _constraints: &TaskConstraints,
        ) -> Result<ValidatorReport, DomainError> {
            ValidatorReport::new("pass", true, "ok", Attributes::empty())
        }
    }

    struct FixedScore;

    #[async_trait]
    impl ScoringPort for FixedScore {
        async fn score(&self, reports: &[ValidatorReport]) -> Result<Score, DomainError> {
            let passed = reports.iter().all(ValidatorReport::passed);
            Score::new(if passed { 1.0 } else { 0.0 })
        }
    }

    struct JsonObjectValidator;

    #[async_trait]
    impl ValidatorPort for JsonObjectValidator {
        fn kind(&self) -> &'static str {
            "json-object"
        }

        async fn validate(
            &self,
            content: &str,
            constraints: &TaskConstraints,
        ) -> Result<ValidatorReport, DomainError> {
            let passed = constraints.output_contract().is_none()
                || serde_json::from_str::<serde_json::Value>(content)
                    .ok()
                    .is_some_and(|value| value.is_object());
            ValidatorReport::new(
                self.kind(),
                passed,
                if passed {
                    "valid object"
                } else {
                    "invalid object"
                },
                Attributes::empty(),
            )
        }
    }

    #[derive(Default)]
    struct NullStats;

    #[async_trait]
    impl choreo_core::ports::StatisticsPort for NullStats {
        async fn record_deliberation(
            &self,
            _specialty: &Specialty,
            _duration: choreo_core::value_objects::DurationMs,
        ) -> Result<(), DomainError> {
            Ok(())
        }

        async fn record_orchestration(
            &self,
            _duration: choreo_core::value_objects::DurationMs,
        ) -> Result<(), DomainError> {
            Ok(())
        }

        async fn snapshot(&self) -> Result<choreo_core::entities::Statistics, DomainError> {
            Ok(choreo_core::entities::Statistics::default())
        }
    }

    struct StubExecutor {
        outcome: ExecutionOutcome,
    }

    #[async_trait]
    impl ExecutorPort for StubExecutor {
        async fn execute(
            &self,
            _winner: &Proposal,
            _options: &Attributes,
        ) -> Result<ExecutionOutcome, DomainError> {
            Ok(self.outcome.clone())
        }
    }

    fn specialty() -> Specialty {
        Specialty::new("triage").unwrap()
    }

    fn task() -> Task {
        Task::new(
            TaskId::new("task-1").unwrap(),
            specialty(),
            TaskDescription::new("decide a remediation").unwrap(),
            TaskConstraints::new(Rubric::empty(), Rounds::new(0).unwrap(), None, None),
            Attributes::empty(),
        )
    }

    fn structured_task() -> Task {
        let constraints =
            TaskConstraints::new(Rubric::empty(), Rounds::new(0).unwrap(), None, None)
                .with_output_contract(
                    OutputContract::json_object(
                        "decision-contract",
                        std::collections::BTreeMap::from([(
                            "decision".to_owned(),
                            OutputFieldRule::new(true, ["emit_event", "escalate"]).unwrap(),
                        )]),
                    )
                    .unwrap(),
                );
        Task::new(
            TaskId::new("task-1").unwrap(),
            specialty(),
            TaskDescription::new("decide a remediation").unwrap(),
            constraints,
            Attributes::empty(),
        )
    }

    fn deliberate_fixture() -> (Arc<DeliberateUseCase>, Arc<RecordingBus>) {
        let specialty = specialty();
        let agent = StubAgent::new("agent-1", &specialty, "apply remediation");
        let council = Council::new(
            CouncilId::new("c1").unwrap(),
            specialty.clone(),
            vec![agent.id().clone()],
            datetime!(2026-04-25 12:00:00 UTC),
        )
        .unwrap();

        let clock = Arc::new(FrozenClock {
            now: datetime!(2026-04-25 12:00:00 UTC),
        });
        let registry = Arc::new(FixedRegistry { council });
        let resolver = Arc::new(StubResolver {
            agent: agent as Arc<dyn AgentPort>,
        });
        let validators: Vec<Arc<dyn ValidatorPort>> = vec![Arc::new(PassValidator)];
        let scoring: Arc<dyn ScoringPort> = Arc::new(FixedScore);
        let repository = Arc::new(InMemoryRepo::default());
        let bus = Arc::new(RecordingBus::default());
        let stats: Arc<dyn choreo_core::ports::StatisticsPort> = Arc::new(NullStats);

        let deliberate = Arc::new(DeliberateUseCase::new(
            clock,
            registry,
            resolver,
            validators,
            scoring,
            repository,
            bus.clone(),
            stats,
            "choreographer",
        ));

        (deliberate, bus)
    }

    fn deliberate_fixture_with_validators(
        agent_draft: &str,
        validators: Vec<Arc<dyn ValidatorPort>>,
    ) -> (Arc<DeliberateUseCase>, Arc<RecordingBus>) {
        let specialty = specialty();
        let agent = StubAgent::new("agent-1", &specialty, agent_draft);
        let council = Council::new(
            CouncilId::new("c1").unwrap(),
            specialty.clone(),
            vec![agent.id().clone()],
            datetime!(2026-04-25 12:00:00 UTC),
        )
        .unwrap();

        let clock = Arc::new(FrozenClock {
            now: datetime!(2026-04-25 12:00:00 UTC),
        });
        let registry = Arc::new(FixedRegistry { council });
        let resolver = Arc::new(StubResolver {
            agent: agent as Arc<dyn AgentPort>,
        });
        let scoring: Arc<dyn ScoringPort> = Arc::new(FixedScore);
        let repository = Arc::new(InMemoryRepo::default());
        let bus = Arc::new(RecordingBus::default());
        let stats: Arc<dyn choreo_core::ports::StatisticsPort> = Arc::new(NullStats);

        let deliberate = Arc::new(DeliberateUseCase::new(
            clock,
            registry,
            resolver,
            validators,
            scoring,
            repository,
            bus.clone(),
            stats,
            "choreographer",
        ));

        (deliberate, bus)
    }

    #[tokio::test]
    async fn unsuccessful_execution_publishes_task_failed_but_returns_output() {
        let (deliberate, bus) = deliberate_fixture();
        let executor = Arc::new(StubExecutor {
            outcome: ExecutionOutcome {
                execution_id: "exec-1".to_owned(),
                succeeded: false,
                duration: DurationMs::from_millis(50),
                output: Attributes::empty(),
            },
        });
        let clock = Arc::new(FrozenClock {
            now: datetime!(2026-04-25 12:00:01 UTC),
        });
        let stats: Arc<dyn choreo_core::ports::StatisticsPort> = Arc::new(NullStats);
        let usecase = OrchestrateUseCase::new(
            deliberate,
            executor,
            bus.clone(),
            clock,
            stats,
            "choreographer",
        );

        let out = usecase.execute(task(), Attributes::empty()).await.unwrap();

        assert!(!out.execution.succeeded);
        assert_eq!(bus.dispatched.lock().unwrap().len(), 1);
        assert_eq!(bus.completed.lock().unwrap().len(), 0);
        assert_eq!(bus.failed.lock().unwrap().len(), 1);
        assert_eq!(out.deliberation.phase(), DeliberationPhase::Completed);
        assert_eq!(out.winner.author(), &AgentId::new("agent-1").unwrap());
    }

    #[tokio::test]
    async fn no_valid_structured_proposal_publishes_task_failed() {
        let (deliberate, bus) =
            deliberate_fixture_with_validators("plain text", vec![Arc::new(JsonObjectValidator)]);
        let executor = Arc::new(StubExecutor {
            outcome: ExecutionOutcome {
                execution_id: "exec-1".to_owned(),
                succeeded: true,
                duration: DurationMs::from_millis(50),
                output: Attributes::empty(),
            },
        });
        let clock = Arc::new(FrozenClock {
            now: datetime!(2026-04-25 12:00:01 UTC),
        });
        let stats: Arc<dyn choreo_core::ports::StatisticsPort> = Arc::new(NullStats);
        let usecase = OrchestrateUseCase::new(
            deliberate,
            executor,
            bus.clone(),
            clock,
            stats,
            "choreographer",
        );

        let err = usecase
            .execute(structured_task(), Attributes::empty())
            .await
            .unwrap_err();

        assert!(matches!(err, DomainError::NoValidProposal { .. }));
        assert_eq!(bus.dispatched.lock().unwrap().len(), 0);
        assert_eq!(bus.failed.lock().unwrap().len(), 1);
        assert_eq!(
            bus.failed.lock().unwrap()[0].error_kind(),
            "deliberation.no_valid_proposal"
        );
    }
}
