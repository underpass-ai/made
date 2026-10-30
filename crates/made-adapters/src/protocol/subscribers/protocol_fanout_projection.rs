use made_core::entities::{AuditRecord, ProtocolEvent, ProtocolInstance};
use made_core::error::DomainError;
use made_core::ports::MetricsRecorderPort;
use made_core::value_objects::{MaxParallel, StreamVersion};

/// One stream's ordered, process-local projection frontier.
pub(super) struct ProtocolFanoutProjection {
    pub(super) version: StreamVersion,
    instance: Option<ProtocolInstance>,
    peak: Option<MaxParallel>,
}

impl Default for ProtocolFanoutProjection {
    fn default() -> Self {
        Self {
            version: StreamVersion::EMPTY,
            instance: None,
            peak: None,
        }
    }
}

impl ProtocolFanoutProjection {
    pub(super) fn observe(
        &mut self,
        record: &AuditRecord,
        metrics: &dyn MetricsRecorderPort,
    ) -> Result<(), DomainError> {
        let event = record.event().ok_or(DomainError::InvariantViolated {
            reason: "fanout metrics require sealed event payloads",
        })?;
        if matches!(
            event,
            ProtocolEvent::TransitionApplied(_)
                | ProtocolEvent::StateIterationStarted(_)
                | ProtocolEvent::ProtocolCompleted(_)
        ) {
            if let (Some(instance), Some(peak)) = (&self.instance, self.peak.take()) {
                metrics.observe_protocol_claim_peak_width(
                    record.definition_name().as_str(),
                    instance.current_state().as_str(),
                    peak,
                );
            }
        }
        match (&mut self.instance, event) {
            (None, ProtocolEvent::ProtocolInstanceStarted(started)) => {
                self.instance = Some(ProtocolInstance::from_started(started));
            }
            (None, ProtocolEvent::InstanceImported(imported)) => {
                self.instance = Some(ProtocolInstance::from_imported(imported));
            }
            (Some(instance), event) => instance.apply(event),
            (None, _) => {
                return Err(DomainError::InvariantViolated {
                    reason: "fanout metrics stream must open before work",
                })
            }
        }
        if let (Some(instance), ProtocolEvent::StepStarted(started)) = (&self.instance, event) {
            let live = instance
                .step_records()
                .values()
                .filter(|step| {
                    step.state_visit() == started.state_visit()
                        && step.state_iteration() == started.state_iteration()
                        && step.has_live_lease_at(started.started_at)
                })
                .count();
            let width = MaxParallel::new(u8::try_from(live).unwrap_or(u8::MAX))?;
            self.peak = Some(self.peak.map_or(width, |peak| peak.max(width)));
        }
        if let ProtocolEvent::StepFailed(failed) = event {
            if let Some(kind) = failed.result.failure_kind() {
                metrics.record_protocol_classified_step_failure(
                    record.definition_name().as_str(),
                    failed.step_id.as_str(),
                    kind,
                );
            }
        }
        self.version = StreamVersion::from_sequence(record.sequence());
        Ok(())
    }
}
