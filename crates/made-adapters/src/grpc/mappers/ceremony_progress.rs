use made_app::usecases::{CeremonyProgressEndReason, CeremonyProgressFrame};
use made_core::error::DomainError;
use made_proto::v1 as pb;

use super::ceremony_history::ceremony_event_record_from;

pub fn stream_ceremony_response_from(
    frame: CeremonyProgressFrame,
) -> Result<pb::StreamCeremonyResponse, DomainError> {
    let frame = match frame {
        CeremonyProgressFrame::Record(record) => {
            pb::stream_ceremony_response::Frame::Record(ceremony_event_record_from(&record)?)
        }
        CeremonyProgressFrame::End(end) => {
            let reason = match end.reason() {
                CeremonyProgressEndReason::Terminal => pb::StreamCeremonyEndReason::Terminal,
                CeremonyProgressEndReason::EventLimit => pb::StreamCeremonyEndReason::EventLimit,
                CeremonyProgressEndReason::WaitElapsed => pb::StreamCeremonyEndReason::WaitElapsed,
            };
            pb::stream_ceremony_response::Frame::End(pb::StreamCeremonyEnd {
                resume_after_sequence: end.resume_after_sequence().value(),
                head_sequence: end.head_sequence().value(),
                reason: reason.into(),
            })
        }
    };
    Ok(pb::StreamCeremonyResponse { frame: Some(frame) })
}
