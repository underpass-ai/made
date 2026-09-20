use made_app::usecases::{CeremonyProgressEndReason, CeremonyProgressFrame};
use made_core::error::DomainError;
use made_core::value_objects::AuditEventType;
use made_proto::v1 as pb;

use super::ceremony_agent_status::status_to_proto;
use super::ceremony_history::ceremony_event_record_from;

pub fn stream_ceremony_response_from(
    frame: CeremonyProgressFrame,
) -> Result<pb::StreamCeremonyResponse, DomainError> {
    let (frame, source) = match frame {
        CeremonyProgressFrame::Record(record) => {
            let source = record_source(record.event_type());
            (
                pb::stream_ceremony_response::Frame::Record(ceremony_event_record_from(&record)?),
                source,
            )
        }
        CeremonyProgressFrame::AgentSnapshot(snapshot) => (
            pb::stream_ceremony_response::Frame::AgentSnapshot(pb::CeremonyAgentActivitySnapshot {
                agents: snapshot.agents().iter().map(status_to_proto).collect(),
                activity_head_sequence: snapshot.activity_head_sequence(),
                complete: snapshot.is_complete(),
            }),
            pb::CeremonyProgressSource::HostAssertion,
        ),
        CeremonyProgressFrame::AgentActivity(activity) => (
            pb::stream_ceremony_response::Frame::AgentActivity(pb::CeremonyAgentActivity {
                sequence: activity.sequence(),
                kind: activity_kind(activity.kind()).into(),
                status: Some(status_to_proto(activity.status())),
            }),
            pb::CeremonyProgressSource::HostAssertion,
        ),
        CeremonyProgressFrame::End(end) => {
            let reason = match end.reason() {
                CeremonyProgressEndReason::Terminal => pb::StreamCeremonyEndReason::Terminal,
                CeremonyProgressEndReason::EventLimit => pb::StreamCeremonyEndReason::EventLimit,
                CeremonyProgressEndReason::WaitElapsed => pb::StreamCeremonyEndReason::WaitElapsed,
            };
            (
                pb::stream_ceremony_response::Frame::End(pb::StreamCeremonyEnd {
                    resume_after_sequence: end.resume_after_sequence().value(),
                    head_sequence: end.head_sequence().value(),
                    reason: reason.into(),
                    resume_after_activity_sequence: end.resume_after_activity_sequence(),
                    activity_head_sequence: end.activity_head_sequence(),
                }),
                pb::CeremonyProgressSource::Unspecified,
            )
        }
    };
    Ok(pb::StreamCeremonyResponse {
        frame: Some(frame),
        source: source.into(),
    })
}

fn record_source(event_type: AuditEventType) -> pb::CeremonyProgressSource {
    match event_type {
        AuditEventType::StepCompleted
        | AuditEventType::StepFailed
        | AuditEventType::ChildCompletionAccepted
        | AuditEventType::ExecutionReceiptLinked => pb::CeremonyProgressSource::AcceptedResult,
        _ => pb::CeremonyProgressSource::EngineTransition,
    }
}

fn activity_kind(
    kind: made_core::entities::CeremonyAgentActivityKind,
) -> pb::CeremonyAgentActivityKind {
    use made_core::entities::CeremonyAgentActivityKind as K;
    match kind {
        K::ActivityChanged => pb::CeremonyAgentActivityKind::ActivityChanged,
        K::BlockerOpened => pb::CeremonyAgentActivityKind::BlockerOpened,
        K::BlockerResolved => pb::CeremonyAgentActivityKind::BlockerResolved,
        K::CheckpointAvailable => pb::CeremonyAgentActivityKind::CheckpointAvailable,
        K::InterventionDelivered => pb::CeremonyAgentActivityKind::InterventionDelivered,
        K::InterventionAnswered => pb::CeremonyAgentActivityKind::InterventionAnswered,
        K::InputRequested => pb::CeremonyAgentActivityKind::InputRequested,
        K::AgentReplaced => pb::CeremonyAgentActivityKind::AgentReplaced,
        K::Stale => pb::CeremonyAgentActivityKind::Stale,
        K::Completed => pb::CeremonyAgentActivityKind::Completed,
        K::Failed => pb::CeremonyAgentActivityKind::Failed,
        K::Heartbeat => pb::CeremonyAgentActivityKind::Heartbeat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_sources_separate_engine_transitions_from_accepted_results() {
        assert_eq!(
            record_source(AuditEventType::TransitionApplied),
            pb::CeremonyProgressSource::EngineTransition
        );
        for accepted in [
            AuditEventType::StepCompleted,
            AuditEventType::StepFailed,
            AuditEventType::ChildCompletionAccepted,
            AuditEventType::ExecutionReceiptLinked,
        ] {
            assert_eq!(
                record_source(accepted),
                pb::CeremonyProgressSource::AcceptedResult
            );
        }
    }
}
