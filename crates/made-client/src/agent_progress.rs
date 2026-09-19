use futures::StreamExt;
use made_proto::v1::{stream_ceremony_response, StreamCeremonyEndReason, StreamCeremonyRequest};

use crate::{
    AgentProgressBatch, AgentProgressFilter, MadeClient, MadeClientError, ProgressCheckpoint,
};

impl MadeClient {
    /// Read one finite batch of sealed engine records plus filtered host activity.
    pub async fn watch_agent_activity_once(
        &self,
        checkpoint: &ProgressCheckpoint,
        after_activity_sequence: u64,
        max_events: u32,
        wait_timeout_ms: Option<u32>,
        filter: &AgentProgressFilter,
    ) -> Result<AgentProgressBatch, MadeClientError> {
        let request = StreamCeremonyRequest {
            ceremony_id: checkpoint.ceremony_id().to_owned(),
            after_sequence: checkpoint.after_sequence(),
            max_events,
            wait_timeout_ms,
            include_agent_activity: true,
            after_activity_sequence,
            role_id: filter.role_id().map(str::to_owned),
            step_id: filter.step_id().map(str::to_owned),
            agent_execution_id: filter.agent_execution_id().map(str::to_owned),
        };
        let mut stream = self
            .rpc()
            .stream_ceremony(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/StreamCeremony",
                request,
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        let mut expected_event = checkpoint.after_sequence().saturating_add(1);
        let mut last_activity = after_activity_sequence;
        let mut records = Vec::new();
        let mut snapshot = None;
        let mut activities = Vec::new();
        while let Some(frame) = stream.next().await {
            let frame = frame.map_err(MadeClientError::from_status)?;
            match frame.frame {
                Some(stream_ceremony_response::Frame::Record(record)) => {
                    if record.sequence != expected_event {
                        return Err(protocol(format!(
                            "expected event sequence {expected_event}, got {}",
                            record.sequence
                        )));
                    }
                    expected_event = expected_event.saturating_add(1);
                    records.push(record);
                }
                Some(stream_ceremony_response::Frame::AgentSnapshot(value)) => {
                    if after_activity_sequence != 0 || snapshot.replace(value).is_some() {
                        return Err(protocol("unexpected duplicate agent snapshot"));
                    }
                }
                Some(stream_ceremony_response::Frame::AgentActivity(activity)) => {
                    if activity.sequence <= last_activity {
                        return Err(protocol("agent activity cursor did not advance"));
                    }
                    last_activity = activity.sequence;
                    activities.push(activity);
                }
                Some(stream_ceremony_response::Frame::End(end)) => {
                    let event_cursor = expected_event.saturating_sub(1);
                    if end.resume_after_sequence != event_cursor
                        || end.resume_after_activity_sequence < last_activity
                    {
                        return Err(protocol(
                            "stream end cursor does not cover delivered frames",
                        ));
                    }
                    let reason = StreamCeremonyEndReason::try_from(end.reason)
                        .map_err(|_| protocol("unknown stream end reason"))?;
                    return Ok(AgentProgressBatch::new(
                        records,
                        snapshot,
                        activities,
                        ProgressCheckpoint::new(checkpoint.ceremony_id(), event_cursor),
                        end.resume_after_activity_sequence,
                        end.head_sequence,
                        end.activity_head_sequence,
                        reason,
                    ));
                }
                None => return Err(protocol("stream returned an empty frame")),
            }
        }
        Err(protocol("stream ended without a resume cursor"))
    }
}

fn protocol(message: impl Into<String>) -> MadeClientError {
    MadeClientError::ProtocolViolation(message.into())
}
