use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use made_core::entities::CeremonyEvent;
use made_core::ports::{CeremonyEventSubscriberPort, MetricsRecorderPort, PositionedRecord};
use made_core::value_objects::{CeremonyOutcome, DurationMs, StepStatus};
use time::OffsetDateTime;

/// Projects sealed ceremony events into the process metrics registry.
pub struct CeremonyMetricsSubscriber {
    metrics: Arc<dyn MetricsRecorderPort>,
    ceremony_starts: Mutex<BTreeMap<String, OffsetDateTime>>,
    step_starts: Mutex<BTreeMap<(String, String, u32, u32), OffsetDateTime>>,
}

impl CeremonyMetricsSubscriber {
    #[must_use]
    pub fn new(metrics: Arc<dyn MetricsRecorderPort>) -> Self {
        Self {
            metrics,
            ceremony_starts: Mutex::new(BTreeMap::new()),
            step_starts: Mutex::new(BTreeMap::new()),
        }
    }

    fn observe_record(&self, positioned: &PositionedRecord) {
        let record = &positioned.record;
        let ceremony = record.definition_name().as_str();
        let Some(event) = record.event() else {
            return;
        };
        match event {
            CeremonyEvent::CeremonyInstanceStarted(started) => {
                self.ceremony_starts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(record.ceremony_id().as_str().to_owned(), started.created_at);
            }
            CeremonyEvent::StepStarted(started) => {
                self.start_step(
                    record.ceremony_id().as_str(),
                    ceremony,
                    started.step_id.as_str(),
                    started.iteration.get(),
                    started.attempt.get(),
                    started.started_at,
                );
            }
            CeremonyEvent::StepCompleted(completed) => {
                self.finish_step(
                    record.ceremony_id().as_str(),
                    ceremony,
                    completed.step_id.as_str(),
                    completed.iteration.get(),
                    completed.attempt.get(),
                    completed.finished_at,
                    StepStatus::Completed,
                );
            }
            CeremonyEvent::StepFailed(failed) => {
                self.finish_step(
                    record.ceremony_id().as_str(),
                    ceremony,
                    failed.step_id.as_str(),
                    failed.iteration.get(),
                    failed.attempt.get(),
                    failed.finished_at,
                    failed.result.status(),
                );
                self.metrics
                    .record_ceremony_outcome(ceremony, CeremonyOutcome::StepFailed);
            }
            CeremonyEvent::StepDeadlineExceeded(exceeded) => {
                self.finish_step(
                    record.ceremony_id().as_str(),
                    ceremony,
                    exceeded.deadline.step_id().as_str(),
                    exceeded.deadline.step_iteration().get(),
                    exceeded.deadline.attempt().get(),
                    exceeded.observed_at,
                    exceeded.result.status(),
                );
                self.metrics
                    .record_ceremony_outcome(ceremony, CeremonyOutcome::StepFailed);
            }
            CeremonyEvent::TransitionApplied(applied) => {
                self.metrics.record_ceremony_transition_applied(
                    ceremony,
                    applied.transition.from_state().as_str(),
                    applied.transition.to_state().as_str(),
                );
            }
            CeremonyEvent::HumanApprovalRecorded(recorded) => {
                self.metrics.record_ceremony_guard_decided(
                    ceremony,
                    recorded.approval.guard_name().as_str(),
                    "approved",
                );
            }
            CeremonyEvent::HumanDeferralRecorded(recorded) => {
                self.metrics.record_ceremony_guard_decided(
                    ceremony,
                    recorded.deferral.guard_name().as_str(),
                    "deferred",
                );
            }
            CeremonyEvent::InterventionRequested(requested) => {
                self.metrics.record_ceremony_intervention_opened(
                    ceremony,
                    requested.intervention.kind().as_label(),
                );
            }
            CeremonyEvent::InterventionResponded(_) => {
                self.metrics.record_ceremony_intervention_answered(ceremony);
            }
            CeremonyEvent::CeremonyCompleted(completed) => {
                self.finish_ceremony(
                    record.ceremony_id().as_str(),
                    ceremony,
                    completed.completed_at,
                );
            }
            CeremonyEvent::ParticipantsBound(_)
            | CeremonyEvent::ContextWritten(_)
            | CeremonyEvent::StateIterationStarted(_)
            | CeremonyEvent::InterventionClosed(_)
            | CeremonyEvent::EvidenceCollected(_)
            | CeremonyEvent::ReasonAsserted(_)
            | CeremonyEvent::InstanceImported(_)
            | CeremonyEvent::MemoryRecalled(_)
            | CeremonyEvent::ChildSpawnPlanned(_)
            | CeremonyEvent::ChildSpawnPlanAdopted(_)
            | CeremonyEvent::ChildCompletionAccepted(_)
            | CeremonyEvent::CeremonyPaused(_)
            | CeremonyEvent::CeremonyResumed(_)
            | CeremonyEvent::CeremonyCancelled(_)
            | CeremonyEvent::CeremonyDeadlineExceeded(_)
            | CeremonyEvent::StateDeadlineExceeded(_)
            | CeremonyEvent::LateStepResultObserved(_)
            | CeremonyEvent::ExecutionReceiptLinked(_) => {}
        }
    }

    fn start_step(
        &self,
        ceremony_id: &str,
        ceremony: &str,
        step: &str,
        iteration: u32,
        attempt: u32,
        started_at: OffsetDateTime,
    ) {
        self.metrics.record_ceremony_step_claimed(ceremony, step);
        self.metrics
            .record_ceremony_step_attempt(ceremony, step, attempt);
        self.metrics
            .record_ceremony_step_iteration(ceremony, step, iteration);
        self.metrics.record_ceremony_lease_acquired(ceremony, step);
        let mut step_starts = self
            .step_starts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let unfinished_lease = step_starts
            .keys()
            .find(
                |(candidate_ceremony, candidate_step, candidate_iteration, _)| {
                    candidate_ceremony == ceremony_id
                        && candidate_step == step
                        && *candidate_iteration == iteration
                },
            )
            .cloned();
        if let Some(expired) = unfinished_lease {
            step_starts.remove(&expired);
            self.metrics.record_ceremony_lease_expired(ceremony, step);
        }
        step_starts.insert(
            (ceremony_id.to_owned(), step.to_owned(), iteration, attempt),
            started_at,
        );
    }

    fn finish_ceremony(&self, ceremony_id: &str, ceremony: &str, finished_at: OffsetDateTime) {
        if let Some(started_at) = self
            .ceremony_starts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(ceremony_id)
        {
            self.metrics
                .observe_ceremony_duration(ceremony, elapsed(started_at, finished_at));
        }
        self.metrics
            .record_ceremony_outcome(ceremony, CeremonyOutcome::Completed);
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_step(
        &self,
        ceremony_id: &str,
        ceremony: &str,
        step: &str,
        iteration: u32,
        attempt: u32,
        finished_at: OffsetDateTime,
        status: StepStatus,
    ) {
        let key = (ceremony_id.to_owned(), step.to_owned(), iteration, attempt);
        if let Some(started_at) = self
            .step_starts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&key)
        {
            self.metrics.observe_ceremony_step_duration(
                ceremony,
                step,
                elapsed(started_at, finished_at),
            );
        }
        self.metrics.record_ceremony_step(ceremony, step, status);
    }
}

impl std::fmt::Debug for CeremonyMetricsSubscriber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CeremonyMetricsSubscriber")
            .field("recorder", &self.metrics.recorder_name())
            .finish()
    }
}

#[async_trait]
impl CeremonyEventSubscriberPort for CeremonyMetricsSubscriber {
    async fn observe(&self, records: &[PositionedRecord]) {
        for record in records {
            self.observe_record(record);
        }
    }
}

fn elapsed(start: OffsetDateTime, finish: OffsetDateTime) -> DurationMs {
    let milliseconds = (finish - start).whole_milliseconds().max(0);
    DurationMs::from_millis(u64::try_from(milliseconds).unwrap_or(u64::MAX))
}
