use std::collections::HashSet;

use futures::StreamExt;
use made_proto::v1::{stream_ceremony_response, StreamCeremonyEndReason, StreamCeremonyRequest};

use crate::{MadeClient, MadeClientError, ProgressBatch, ProgressCheckpoint};

impl MadeClient {
    pub async fn watch_once(
        &self,
        checkpoint: &ProgressCheckpoint,
        max_events: u32,
        wait_timeout_ms: Option<u32>,
    ) -> Result<ProgressBatch, MadeClientError> {
        let mut rpc = self.rpc();
        let response = rpc
            .stream_ceremony(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/StreamCeremony",
                StreamCeremonyRequest {
                    ceremony_id: checkpoint.ceremony_id().to_owned(),
                    after_sequence: checkpoint.after_sequence(),
                    max_events,
                    wait_timeout_ms,
                },
            ))
            .await
            .map_err(MadeClientError::from_status)?;
        let mut stream = response.into_inner();
        let mut expected_sequence = checkpoint.after_sequence().saturating_add(1);
        let mut event_ids = HashSet::new();
        let mut records = Vec::new();

        while let Some(frame) = stream.next().await {
            let frame = frame.map_err(MadeClientError::from_status)?;
            match frame.frame {
                Some(stream_ceremony_response::Frame::Record(record)) => {
                    if record.ceremony_id != checkpoint.ceremony_id() {
                        return Err(MadeClientError::ProtocolViolation(format!(
                            "stream returned ceremony {} for scope {}",
                            record.ceremony_id,
                            checkpoint.ceremony_id()
                        )));
                    }
                    if !event_ids.insert(record.event_id.clone()) {
                        continue;
                    }
                    if record.sequence != expected_sequence {
                        return Err(MadeClientError::ProtocolViolation(format!(
                            "expected event sequence {expected_sequence}, got {}",
                            record.sequence
                        )));
                    }
                    expected_sequence = expected_sequence.saturating_add(1);
                    records.push(record);
                }
                Some(stream_ceremony_response::Frame::End(end)) => {
                    let last_seen = expected_sequence.saturating_sub(1);
                    if end.resume_after_sequence != last_seen {
                        return Err(MadeClientError::ProtocolViolation(format!(
                            "end cursor {} does not match last sequence {last_seen}",
                            end.resume_after_sequence
                        )));
                    }
                    let reason = StreamCeremonyEndReason::try_from(end.reason).map_err(|_| {
                        MadeClientError::ProtocolViolation(format!(
                            "unknown stream end reason {}",
                            end.reason
                        ))
                    })?;
                    return Ok(ProgressBatch::new(
                        records,
                        ProgressCheckpoint::new(
                            checkpoint.ceremony_id(),
                            end.resume_after_sequence,
                        ),
                        end.head_sequence,
                        reason,
                    ));
                }
                None => {
                    return Err(MadeClientError::ProtocolViolation(
                        "stream returned an empty frame".to_owned(),
                    ));
                }
            }
        }

        Err(MadeClientError::ProtocolViolation(
            "stream ended without a resume cursor".to_owned(),
        ))
    }
}
