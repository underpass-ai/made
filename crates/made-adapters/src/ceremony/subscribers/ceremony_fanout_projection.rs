use made_core::entities::{AuditRecord, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::MetricsRecorderPort;
use made_core::value_objects::{MaxParallel, StreamVersion};

/// One stream's ordered, process-local projection frontier.
pub(super) struct CeremonyFanoutProjection {
    pub(super) version: StreamVersion,
    instance: Option<CeremonyInstance>,
    peak: Option<MaxParallel>,
}

impl Default for CeremonyFanoutProjection {
    fn default() -> Self {
        Self {
            version: StreamVersion::EMPTY,
            instance: None,
            peak: None,
        }
    }
}

impl CeremonyFanoutProjection {
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
            CeremonyEvent::TransitionApplied(_)
                | CeremonyEvent::StateIterationStarted(_)
                | CeremonyEvent::CeremonyCompleted(_)
        ) {
            if let (Some(instance), Some(peak)) = (&self.instance, self.peak.take()) {
                metrics.observe_ceremony_claim_peak_width(
                    record.definition_name().as_str(),
                    instance.current_state().as_str(),
                    peak,
                );
            }
        }
        match (&mut self.instance, event) {
            (None, CeremonyEvent::CeremonyInstanceStarted(started)) => {
                self.instance = Some(CeremonyInstance::from_started(started));
            }
            (None, CeremonyEvent::InstanceImported(imported)) => {
                self.instance = Some(CeremonyInstance::from_imported(imported));
            }
            (Some(instance), event) => instance.apply(event),
            (None, _) => {
                return Err(DomainError::InvariantViolated {
                    reason: "fanout metrics stream must open before work",
                })
            }
        }
        if let (Some(instance), CeremonyEvent::StepStarted(started)) = (&self.instance, event) {
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
        if let CeremonyEvent::StepFailed(failed) = event {
            if let Some(kind) = failed.result.failure_kind() {
                metrics.record_ceremony_classified_step_failure(
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
