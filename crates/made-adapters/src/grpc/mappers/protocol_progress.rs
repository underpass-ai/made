use made_app::usecases::{ProtocolProgressEndReason, ProtocolProgressFrame};
use made_core::error::DomainError;
use made_core::value_objects::AuditEventType;
use made_proto::v1 as pb;

use super::protocol_agent_status::status_to_proto;
use super::protocol_history::protocol_event_record_from;

pub fn stream_protocol_response_from(
    frame: ProtocolProgressFrame,
) -> Result<pb::StreamProtocolResponse, DomainError> {
    let (frame, source) = match frame {
        ProtocolProgressFrame::Record(record) => {
            let source = record_source(record.event_type());
            (
                pb::stream_protocol_response::Frame::Record(protocol_event_record_from(&record)?),
                source,
            )
        }
        ProtocolProgressFrame::AgentSnapshot(snapshot) => (
            pb::stream_protocol_response::Frame::AgentSnapshot(pb::ProtocolAgentActivitySnapshot {
                agents: snapshot.agents().iter().map(status_to_proto).collect(),
                activity_head_sequence: snapshot.activity_head_sequence(),
                complete: snapshot.is_complete(),
            }),
            pb::ProtocolProgressSource::HostAssertion,
        ),
        ProtocolProgressFrame::AgentActivity(activity) => (
            pb::stream_protocol_response::Frame::AgentActivity(pb::ProtocolAgentActivity {
                sequence: activity.sequence(),
                kind: activity_kind(activity.kind()).into(),
                status: Some(status_to_proto(activity.status())),
            }),
            pb::ProtocolProgressSource::HostAssertion,
        ),
        ProtocolProgressFrame::End(end) => {
            let reason = match end.reason() {
                ProtocolProgressEndReason::Terminal => pb::StreamProtocolEndReason::Terminal,
                ProtocolProgressEndReason::EventLimit => pb::StreamProtocolEndReason::EventLimit,
                ProtocolProgressEndReason::WaitElapsed => pb::StreamProtocolEndReason::WaitElapsed,
            };
            (
                pb::stream_protocol_response::Frame::End(pb::StreamProtocolEnd {
                    resume_after_sequence: end.resume_after_sequence().value(),
                    head_sequence: end.head_sequence().value(),
                    reason: reason.into(),
                    resume_after_activity_sequence: end.resume_after_activity_sequence(),
                    activity_head_sequence: end.activity_head_sequence(),
                }),
                pb::ProtocolProgressSource::Unspecified,
            )
        }
    };
    Ok(pb::StreamProtocolResponse {
        frame: Some(frame),
        source: source.into(),
    })
}

fn record_source(event_type: AuditEventType) -> pb::ProtocolProgressSource {
    match event_type {
        AuditEventType::StepCompleted
        | AuditEventType::StepFailed
        | AuditEventType::ChildCompletionAccepted
        | AuditEventType::ExecutionReceiptLinked => pb::ProtocolProgressSource::AcceptedResult,
        _ => pb::ProtocolProgressSource::EngineTransition,
    }
}

fn activity_kind(
    kind: made_core::entities::ProtocolAgentActivityKind,
) -> pb::ProtocolAgentActivityKind {
    use made_core::entities::ProtocolAgentActivityKind as K;
    match kind {
        K::ActivityChanged => pb::ProtocolAgentActivityKind::ActivityChanged,
        K::BlockerOpened => pb::ProtocolAgentActivityKind::BlockerOpened,
        K::BlockerResolved => pb::ProtocolAgentActivityKind::BlockerResolved,
        K::CheckpointAvailable => pb::ProtocolAgentActivityKind::CheckpointAvailable,
        K::InterventionDelivered => pb::ProtocolAgentActivityKind::InterventionDelivered,
        K::InterventionAnswered => pb::ProtocolAgentActivityKind::InterventionAnswered,
        K::InputRequested => pb::ProtocolAgentActivityKind::InputRequested,
        K::AgentReplaced => pb::ProtocolAgentActivityKind::AgentReplaced,
        K::Stale => pb::ProtocolAgentActivityKind::Stale,
        K::Completed => pb::ProtocolAgentActivityKind::Completed,
        K::Failed => pb::ProtocolAgentActivityKind::Failed,
        K::Heartbeat => pb::ProtocolAgentActivityKind::Heartbeat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_sources_separate_engine_transitions_from_accepted_results() {
        assert_eq!(
            record_source(AuditEventType::TransitionApplied),
            pb::ProtocolProgressSource::EngineTransition
        );
        for accepted in [
            AuditEventType::StepCompleted,
            AuditEventType::StepFailed,
            AuditEventType::ChildCompletionAccepted,
            AuditEventType::ExecutionReceiptLinked,
        ] {
            assert_eq!(
                record_source(accepted),
                pb::ProtocolProgressSource::AcceptedResult
            );
        }
    }
}
