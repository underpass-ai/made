use made_core::entities::ProtocolEvent;

use super::step_about;

/// What a step-shaped event is *about*.
///
/// Six arms that all name the same thing — a step at a point in its
/// state's history — and one that names a renewal request instead. They
/// live together because the coordinates are the identity: two attempts
/// of one step in one visit are two facts, and a spelling that dropped
/// any of them would make a retry replay as the first try.
pub(super) fn step_execution_about(event: &ProtocolEvent) -> String {
    match event {
        ProtocolEvent::StepStarted(started) => step_about(
            &started.step_id,
            started.state_visit,
            started.state_iteration().get(),
            started.iteration.get(),
            started.attempt.get(),
        ),
        ProtocolEvent::StepLeaseRenewed(renewed) if renewed.request.is_some() => format!(
            "renewal:{}",
            renewed
                .request
                .as_ref()
                .expect("checked request")
                .id
                .as_str()
        ),
        ProtocolEvent::StepLeaseRenewed(renewed) => format!(
            "step:{}:claim:{}:expiry:{}",
            renewed.step_id,
            renewed.claim_fence.as_str(),
            renewed.expires_at.unix_timestamp_nanos()
        ),
        ProtocolEvent::StepCompleted(completed) => step_about(
            &completed.step_id,
            completed.state_visit,
            completed.state_iteration().get(),
            completed.iteration.get(),
            completed.attempt.get(),
        ),
        ProtocolEvent::StepFailed(failed) => step_about(
            &failed.step_id,
            failed.state_visit,
            failed.state_iteration().get(),
            failed.iteration.get(),
            failed.attempt.get(),
        ),
        ProtocolEvent::ContextWritten(written) => step_about(
            &written.step_id,
            written.state_visit,
            written.state_iteration.get(),
            written.iteration.get(),
            written.attempt.get(),
        ),
        ProtocolEvent::StateIterationStarted(started) => {
            let visit = started
                .state_visit
                .map(|visit| format!(":visit:{}", visit.get()))
                .unwrap_or_default();
            format!(
                "state:{}{visit}:iteration:{}",
                started.state_id,
                started.state_iteration.get()
            )
        }
        _ => unreachable!("only step-shaped events are delegated here"),
    }
}
