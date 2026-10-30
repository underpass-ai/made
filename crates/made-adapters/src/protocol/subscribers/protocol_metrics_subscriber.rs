use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use made_core::entities::ProtocolEvent;
use made_core::ports::{MetricsRecorderPort, PositionedRecord, ProtocolEventSubscriberPort};
use made_core::value_objects::{DurationMs, ProtocolOutcome, StepStatus};
use time::OffsetDateTime;

/// Projects sealed protocol events into the process metrics registry.
pub struct ProtocolMetricsSubscriber {
    metrics: Arc<dyn MetricsRecorderPort>,
    protocol_starts: Mutex<BTreeMap<String, OffsetDateTime>>,
    step_starts: Mutex<BTreeMap<(String, String, u32, u32), OffsetDateTime>>,
}

impl ProtocolMetricsSubscriber {
    #[must_use]
    pub fn new(metrics: Arc<dyn MetricsRecorderPort>) -> Self {
        Self {
            metrics,
            protocol_starts: Mutex::new(BTreeMap::new()),
            step_starts: Mutex::new(BTreeMap::new()),
        }
    }

    fn observe_record(&self, positioned: &PositionedRecord) {
        let record = &positioned.record;
        let protocol = record.definition_name().as_str();
        let Some(event) = record.event() else {
            return;
        };
        match event {
            ProtocolEvent::ProtocolInstanceStarted(started) => {
                self.protocol_starts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(record.protocol_id().as_str().to_owned(), started.created_at);
            }
            ProtocolEvent::StepStarted(_)
            | ProtocolEvent::StepCompleted(_)
            | ProtocolEvent::StepFailed(_)
            | ProtocolEvent::StepDeadlineExceeded(_) => {
                self.observe_step_record(record.protocol_id().as_str(), protocol, event);
            }
            ProtocolEvent::TransitionApplied(applied) => {
                self.metrics.record_protocol_transition_applied(
                    protocol,
                    applied.transition.from_state().as_str(),
                    applied.transition.to_state().as_str(),
                );
            }
            ProtocolEvent::HumanApprovalRecorded(recorded) => {
                self.metrics.record_protocol_guard_decided(
                    protocol,
                    recorded.approval.guard_name().as_str(),
                    "approved",
                );
            }
            ProtocolEvent::HumanDeferralRecorded(recorded) => {
                self.metrics.record_protocol_guard_decided(
                    protocol,
                    recorded.deferral.guard_name().as_str(),
                    "deferred",
                );
            }
            ProtocolEvent::InterventionRequested(requested) => {
                self.metrics.record_protocol_intervention_opened(
                    protocol,
                    requested.intervention.kind().as_label(),
                );
            }
            ProtocolEvent::InterventionResponded(_) => {
                self.metrics.record_protocol_intervention_answered(protocol);
            }
            ProtocolEvent::ProtocolCompleted(completed) => {
                self.finish_protocol(
                    record.protocol_id().as_str(),
                    protocol,
                    completed.completed_at,
                );
            }
            ProtocolEvent::ParticipantsBound(_)
            | ProtocolEvent::StepLeaseRenewed(_)
            | ProtocolEvent::HostHandoffRecorded(_)
            | ProtocolEvent::ContextWritten(_)
            | ProtocolEvent::StateIterationStarted(_)
            | ProtocolEvent::InterventionClosed(_)
            | ProtocolEvent::InterventionDeliveryAcknowledged(_)
            | ProtocolEvent::EvidenceCollected(_)
            | ProtocolEvent::ReasonAsserted(_)
            | ProtocolEvent::InstanceImported(_)
            | ProtocolEvent::MemoryRecalled(_)
            | ProtocolEvent::ChildSpawnPlanned(_)
            | ProtocolEvent::ChildSpawnPlanAdopted(_)
            | ProtocolEvent::ChildCompletionAccepted(_)
            | ProtocolEvent::ProtocolPaused(_)
            | ProtocolEvent::ProtocolResumed(_)
            | ProtocolEvent::ProtocolCancelled(_)
            | ProtocolEvent::ProtocolDeadlineExceeded(_)
            | ProtocolEvent::StateDeadlineExceeded(_)
            | ProtocolEvent::LateStepResultObserved(_)
            | ProtocolEvent::ExecutionReceiptLinked(_)
            | ProtocolEvent::SuccessorPlanned(_)
            | ProtocolEvent::SuccessionCarried(_) => {}
        }
    }

    fn observe_step_record(&self, protocol_id: &str, protocol: &str, event: &ProtocolEvent) {
        match event {
            ProtocolEvent::StepStarted(started) => self.start_step(
                protocol_id,
                protocol,
                started.step_id.as_str(),
                started.iteration.get(),
                started.attempt.get(),
                started.started_at,
            ),
            ProtocolEvent::StepCompleted(completed) => self.finish_step(
                protocol_id,
                protocol,
                completed.step_id.as_str(),
                completed.iteration.get(),
                completed.attempt.get(),
                completed.finished_at,
                StepStatus::Completed,
            ),
            ProtocolEvent::StepFailed(failed) => {
                self.finish_step(
                    protocol_id,
                    protocol,
                    failed.step_id.as_str(),
                    failed.iteration.get(),
                    failed.attempt.get(),
                    failed.finished_at,
                    failed.result.status(),
                );
                self.metrics
                    .record_protocol_outcome(protocol, ProtocolOutcome::StepFailed);
            }
            ProtocolEvent::StepDeadlineExceeded(exceeded) => {
                self.finish_step(
                    protocol_id,
                    protocol,
                    exceeded.deadline.step_id().as_str(),
                    exceeded.deadline.step_iteration().get(),
                    exceeded.deadline.attempt().get(),
                    exceeded.observed_at,
                    exceeded.result.status(),
                );
                self.metrics
                    .record_protocol_outcome(protocol, ProtocolOutcome::StepFailed);
            }
            _ => unreachable!("only step events are delegated here"),
        }
    }

    fn start_step(
        &self,
        protocol_id: &str,
        protocol: &str,
        step: &str,
        iteration: u32,
        attempt: u32,
        started_at: OffsetDateTime,
    ) {
        self.metrics.record_protocol_step_claimed(protocol, step);
        self.metrics
            .record_protocol_step_attempt(protocol, step, attempt);
        self.metrics
            .record_protocol_step_iteration(protocol, step, iteration);
        self.metrics.record_protocol_lease_acquired(protocol, step);
        let mut step_starts = self
            .step_starts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let unfinished_lease = step_starts
            .keys()
            .find(
                |(candidate_protocol, candidate_step, candidate_iteration, _)| {
                    candidate_protocol == protocol_id
                        && candidate_step == step
                        && *candidate_iteration == iteration
                },
            )
            .cloned();
        if let Some(expired) = unfinished_lease {
            step_starts.remove(&expired);
            self.metrics.record_protocol_lease_expired(protocol, step);
        }
        step_starts.insert(
            (protocol_id.to_owned(), step.to_owned(), iteration, attempt),
            started_at,
        );
    }

    fn finish_protocol(&self, protocol_id: &str, protocol: &str, finished_at: OffsetDateTime) {
        if let Some(started_at) = self
            .protocol_starts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(protocol_id)
        {
            self.metrics
                .observe_protocol_duration(protocol, elapsed(started_at, finished_at));
        }
        self.metrics
            .record_protocol_outcome(protocol, ProtocolOutcome::Completed);
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_step(
        &self,
        protocol_id: &str,
        protocol: &str,
        step: &str,
        iteration: u32,
        attempt: u32,
        finished_at: OffsetDateTime,
        status: StepStatus,
    ) {
        let key = (protocol_id.to_owned(), step.to_owned(), iteration, attempt);
        if let Some(started_at) = self
            .step_starts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&key)
        {
            self.metrics.observe_protocol_step_duration(
                protocol,
                step,
                elapsed(started_at, finished_at),
            );
        }
        self.metrics.record_protocol_step(protocol, step, status);
    }
}

impl std::fmt::Debug for ProtocolMetricsSubscriber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProtocolMetricsSubscriber")
            .field("recorder", &self.metrics.recorder_name())
            .finish()
    }
}

#[async_trait]
impl ProtocolEventSubscriberPort for ProtocolMetricsSubscriber {
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
